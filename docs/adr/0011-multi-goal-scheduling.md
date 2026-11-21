# 0011 — Varios objetivos a la vez, con reparto justo y claims de rutas

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

El operador encoló dos objetivos y observó que el primero acaparaba la ejecución. Además,
un plan de 7 tareas independientes se ejecutaba de una en una, y con varios agentes
escribiendo aparecían errores `database is locked`.

El requisito es: varios objetivos avanzando a la vez; si interfieren, el orquestador decide
qué esperar. La decisión debe ser durable (sobrevive a reinicios) y no puede convertir un
fallo de concurrencia en un fallo de tarea.

## Alternativas consideradas

1. **Un objetivo a la vez.** Simple y predecible, pero desperdicia la capacidad y contradice
   el requisito.
2. **FIFO global de tareas.** Llena los huecos, pero un objetivo con muchas tareas puede
   monopolizar todos los permisos y los demás no avanzan.
3. **Límite fijo por objetivo.** Justo, pero si hay un solo objetivo desaprovecha la mitad de
   la capacidad; si hay cinco, unos esperan sin razón.
4. **Reparto dinámico + rotación + claims.** Cada objetivo recibe
   `max_concurrent / objetivos_activos` (mínimo 1), el orden rota en cada tick y las rutas
   declaradas por el planificador impiden que dos tareas toquen los mismos archivos.

## Decisión

- El scheduler despacha **hasta el cupo de cada objetivo** en cada tick, no solo cuando no hay
  nada en vuelo (se eliminó la condición «un run a la vez por objetivo»).
- El cupo es dinámico: `max_concurrent / objetivos_activos`, mínimo 1. El orden de objetivos
  rota para que el más antiguo no gane siempre.
- Los **claims de rutas** se comparten entre objetivos del mismo proyecto durante el tick, de
  modo que dos objetivos no arrancan tareas solapadas por una carrera. Los claims vienen del
  planificador; una tarea sin claims se considera en conflicto con todo lo que corre.
- El **techo de presupuesto** solo impide despachar trabajo nuevo. Un objetivo con tareas en
  vuelo no se bloquea por haber superado el techo: se deja terminar y se corta el siguiente
  despacho.
- Los conflictos de integración siguen resolviéndose en la cola de merge: rebase sobre la
  base actual, re-ejecución de gates y fast-forward; un conflicto bloquea el objetivo, no lo
  falla.
- **SQLite** usa una única conexión en el pool. Varias transacciones de este código empiezan
  leyendo y luego escriben; SQLite no espera a una transacción diferida que se convierte en
  escritora y devuelve `SQLITE_BUSY` sin llamar al busy handler. SQLite ya tiene un solo
  escritor, así que serializar por conexión elimina la carrera sin cambiar semántica.

## Consecuencias

- **A favor:** varios objetivos progresan; el reparto es observable en la UI; una tarea que
  espera dice por qué; el presupuesto no mata trabajo en curso; no hay locks bajo carga.
- **En contra:** si un objetivo está planificando o bloqueado por dependencias, su cupo puede
  quedar ocioso hasta el siguiente tick; los planes sin claims se serializan (conservador);
  la concurrencia total sigue acotada por `max_concurrent`, que requiere reinicio.
- La UI calcula el motivo de espera a partir del snapshot (dependencias, claims, capacidad);
  el daemon sigue siendo la autoridad y puede despachar en cuanto haya hueco.

## Verificación

- `just e2e-multigoal`: dos objetivos, un proyecto, cuatro permisos; ambos deben tener tareas
  en ejecución a la vez, ninguno debe fallar por lock y ambos deben terminar `succeeded`.
- `just soak`: el techo bloquea el segundo objetivo y el primero termina; reanudar completa
  el segundo.
- `cargo test -p jcs-store concurrent_appends_do_not_lock_the_database`: ocho escritores
  concurrentes sobre un archivo SQLite real.
- `cargo test -p jcs-core claims`: solapamiento por segmentos y claims vacíos.
