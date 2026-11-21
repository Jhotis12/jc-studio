# 0013 — Progreso por etapas y tareas iterables

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

La interfaz mostraba un porcentaje por tarea calculado a partir del estado: 45 % mientras el
agente trabajaba y 100 % al terminar. El operador lo detectó como una mentira: la barra
saltaba de 45 a 100 sin relación con el trabajo restante. Además, el progreso del objetivo
usaba un único fill que se volvía azul en cuanto una tarea estaba en curso, aunque solo una
de seis hubiera terminado.

Por otro lado, una tarea terminada o bloqueada no tenía camino de vuelta: si el resultado no
gustaba, no había forma de pedir cambios. Y un objetivo con una tarea pendiente que dependía
de una bloqueada se quedaba `running` para siempre porque `settled()` era falso.

## Alternativas consideradas

1. **Progreso por tiempo restante del timeout.** Fácil, pero una tarea de 30 s mostraría 3 %
   y saltaría a 100 %; peor que el estado actual.
2. **Progreso por tokens.** No hay un total conocido y varía por modelo; no es comparable.
3. **Etapas discretas + barra segmentada.** Honesto: la UI dice en qué fase está y cuántas
   fases ha completado, sin inventar un porcentaje.
4. **Reabrir la tarea terminada.** Violaría el invariante de que `succeeded` es terminal y
   mezclaría el trabajo ya integrado con cambios nuevos.
5. **Nueva tarea dependiente con feedback.** Conserva el historial, usa la rama de
   integración como base y deja que los gates vuelvan a decidir.

## Decisión

- `TaskStage`: en cola, agente trabajando, verificando, integrando, completada, merge
  bloqueado, falló, bloqueada, cancelada. `TaskProgress` lleva `filled`/`total` (4 segmentos),
  nunca un porcentaje.
- Las tareas en curso muestran un spinner; el objetivo usa una tira de un segmento por tarea
  coloreada por etapa, además de los conteos. El fill único desaparece.
- Las instrucciones del plan se persisten en `tasks.instructions` (migración 0007) y se
  incluyen en el prompt del ejecutor. Antes solo se veía el título.
- `POST /tasks/{id}/iterate`:
  - failed/blocked → `instructions` nuevas y vuelta a `pending` (mismo worktree);
  - succeeded → nueva tarea dependiente con el feedback, cortada de la rama de integración;
  - running/verifying → 409, hay que esperar o abortar.
- `POST /tasks/{id}/merge-retry` limpia `merge_error` para que la cola reintente tras
  resolver el conflicto en el worktree.
- Detección de estancamiento: si el objetivo no tiene tareas en vuelo, `ready_tasks` está
  vacío, no hay merges pendientes y quedan pendientes, se marca `blocked`/`failed` y se
  registra `goal_stalled`. `settled()` solo no bastaba.
- Los eventos idénticos consecutivos se pliegan en la consola con un contador `×N`; además,
  `ensure_integration_branch` solo emite evento cuando crea o mueve la rama.

## Consecuencias

- **A favor:** el operador ve la fase real; un objetivo no se queda colgado; una tarea se
  puede corregir sin reescribir la historia; el feedback queda persistido y auditable.
- **En contra:** una tarea `succeeded` con merge fallido no se puede reintentar en su sitio:
  se crea una tarea dependiente o se resuelve el merge. Los porcentajes desaparecen de la
  UI, que es intencional.
- Las instrucciones del plan ocupan espacio en la base; es texto pequeño y evita el problema
  de prompts que solo llevaban el título.

## Verificación

- `cargo test -p jcs-store iteration_tests`: crea la tarea dependiente, guarda feedback y
  reintenta una bloqueada.
- `a_finished_task_can_be_iterated_and_a_merge_retried` (HTTP): crea la iteración y limpia el
  error de merge.
- El goal real `goal_c13231…` pasó de `running` a `blocked` con `goal_stalled` al arrancar el
  daemon nuevo.
- Inspección visual de la app: segmentos por etapa, sin 45 % ni fill azul global.
