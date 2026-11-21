# 0008 — Orquestador durable: plan, DAG, leases y rama de integración

- **Estado:** Accepted
- **Fecha:** 2026-11-21
- **Complementa a:** [0001](0001-daemon-thin-client.md), [0004](0004-policy-as-code-with-gates.md)

## Contexto

Una tarea por objetivo no es orquestación. Un objetivo real ("añade validación de email y
sus tests") se descompone en trabajo que tiene orden, que puede solaparse en el tiempo y que
debe sobrevivir a un reinicio. Las decisiones caras de revertir son cuatro:

1. cómo se convierte un objetivo en tareas sin confiar en la prosa de un modelo;
2. cómo se ejecuta un DAG sin que dos agentes se pisen ni se dupliquen tras un crash;
3. cómo ve una tarea dependiente el trabajo de su predecesora;
4. cuándo y cómo se integra el resultado en la rama del usuario.

## Alternativas consideradas

**Descomposición**
1. *Pedir al modelo una lista y parsear el texto.* Frágil: cualquier frase de más rompe el
   plan o, peor, produce un plan silenciosamente incompleto.
2. *Plantillas fijas por tipo de objetivo.* Predecible y limitado: no cubre objetivos nuevos.
3. *JSON estricto + validación de dominio + una reparación.* El modelo propone; el dominio
   decide si la propuesta es un plan.

**Ejecución**
4. *Un solo working tree con checkout secuencial.* Sin paralelismo y con pérdida de trabajo si
   algo falla a mitad.
5. *Worktree por tarea (ya decidido en ADR 0005) + leases en SQLite.* Un dueño por tarea, con
   heartbeat y reclamación de leases expirados.

**Integración**
6. *Cada tarea directamente sobre la rama base.* Un dependiente no vería a su predecesor.
7. *Rama de integración por objetivo.* Los worktrees se cortan de ella, las tareas se mergean
   ahí en orden, y solo al final se promociona (o no) a la base.

## Decisión

**Plan.** El planificador es una sesión de agente con un prompt que exige **solo** un objeto
JSON. El daemon extrae el primer objeto balanceado (ignora markdown y prosa), lo valida contra
las reglas del dominio (claves únicas, dependencias existentes, sin ciclos, rutas relativas,
máximo 12 tareas) y, si falla, **una** reparación con el error concreto. Se guarda el plan
verbatim en `plans` y se materializa en `tasks` + `task_deps` en **una transacción**.

**Scheduler.** Un bucle cada segundo que solo consulta estado durable:
- `expire_leases` + auditoría;
- integra tareas pendientes de merge **antes** de despachar;
- para cada objetivo activo: presupuesto → conteos → finalización → despacho.

La disponibilidad es SQL (`ready_tasks`): pendiente y con **todas** sus dependencias
`succeeded` **y ya integradas** en la rama de integración del objetivo. Un dependiente nunca
se corta de una rama que no contiene a su predecesor.

**Leases.** El scheduler adquiere `task:<id>` antes de despachar; el ejecutor renueva cada 20 s
y **aborta si la pierde** (otro dueño es responsable). Un lease sin renovar caduca (90 s) y se
reclama, que es exactamente lo que debe pasar tras `kill -9`.

**Aislamiento entre tareas.** Cada tarea declara `paths` (prefijos relativos). Dos tareas con
claims solapados nunca corren a la vez. Una tarea sin claims espera a que no haya nada en
vuelo: no saber qué se toca no es permiso para tocar todo.

**Integración.** Cada objetivo con más de una tarea (o con `auto_merge`) recibe una rama
`jcs/goal/<id>` creada desde la base. Los worktrees de tarea se cortan de ahí. Al terminar una
tarea, la cola de merge:
1. rebasa la rama de la tarea sobre la integración (dentro de su worktree);
2. **vuelve a ejecutar los gates** sobre el resultado rebasado;
3. hace `merge --ff-only` en un worktree dedicado de la integración;
4. borra worktree y rama de la tarea.

`auto_merge` controla **solo** el último paso (integración → base), que exige árbol limpio,
rama base chequeada y fast-forward. Nunca hay force-push, ni merge con conflictos, ni
resolución automática.

**Estado del objetivo.** Lo deriva el scheduler de los conteos de tareas **y** del estado de
merge. Una tarea nunca escribe el estado del objetivo: con varias tareas, la última en
terminar no tiene autoridad sobre el conjunto.

**Presupuestos.** Ventana diaria en UTC sobre `runs`. Al agotarse, el objetivo pasa a
`Blocked` con el motivo, no se despacha nada más, y `jcs resume` lo reactiva cuando el
operador decide (o cuando la ventana se reinicia).

## Consecuencias

**Positivas**
- Un crash no duplica trabajo: leases y `attempt` hacen visible quién hizo qué.
- Los dependientes construyen sobre código real, no sobre una foto antigua de la base.
- Un conflicto de merge es un `Blocked` explícito, no un commit a medias.
- El coste diario es un techo, no una sorpresa.

**Negativas / costes**
- El rebase + re-verificación duplica el coste de gates en cadenas largas. Es el precio de
  que el veredicto corresponda al código que se integra.
- Un plan mal descompuesto se paga entero: por eso el plan se valida y se limita a 12 tareas.
- La rama de integración es un objeto más que el operador debe entender; `jcs goal` lo muestra.
- `auto_merge` toca la rama del usuario por diseño: exige árbol limpio y falla en voz alta.

**Riesgos mitigados**
- Carrera "objetivo terminado antes que sus merges": `goal_pending_merges` bloquea la
  finalización hasta que la integración está completa.
- Carrera "cliente cierra antes del fin del run": el run se cierra **antes** de que la tarea
  pase a terminal, porque el estado de la tarea es lo que dispara la finalización del objetivo.
- Plan con ciclo o rutas que escapan: rechazado antes de escribir una sola fila.

## Verificación

- `jcs-core::plan`: claves, ciclos, rutas, límites (unit tests + property tests del orden).
- `jcs-store`: materialización transaccional, `ready_tasks` con dependencias integradas,
  leases (exclusividad, caducidad, reclamación), conteos y merges pendientes.
- `scripts/e2e-plan.sh`: un objetivo se descompone en ≥2 tareas con dependencias, todas
  terminan y todos los entregables existen.
- `scripts/e2e-merge.sh`: con `auto_merge`, la rama base acaba conteniendo todo el trabajo y
  no queda ninguna rama `jcs/*`.
- `scripts/chaos.sh`: `kill -9` a mitad de run → `interrupted` + `blocked`, sin commits
  parciales, y el daemon sigue funcionando.
