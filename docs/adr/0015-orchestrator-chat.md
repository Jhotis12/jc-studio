# 0015 — Chat del Orquestador separado de la ejecución de objetivos

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

El operador escribió «Hola» en el compositor de objetivos. El planificador recibió el texto
como un objetivo de ingeniería y, con acceso al repositorio, leyó `openspec/changes/...` y
fabricó 4 tareas sobre trabajo pendiente no relacionado. Se ejecutaron 3 agentes y se gastó
dinero antes de cancelarlo.

El compositor no distingue una conversación de una orden de trabajo, y el planificador no
tenía una salida explícita para «esto no es un objetivo».

## Alternativas consideradas

1. **Confiar en el prompt del planificador** para que no invente. Es necesario, pero no
   suficiente: un modelo con acceso al repo puede racionalizar cualquier texto.
2. **Rechazar mensajes cortos.** Rompe objetivos legítimos como «arregla CI».
3. **Un chat conversacional separado y un guard exacto de saludos.** El chat es el lugar
   natural para preguntar; el compositor queda para trabajo concreto.

## Decisión

- El proyecto tiene un **chat del Orquestador** persistente (`orchestrator_messages`,
  `orchestrator_sessions`, migración 0009). Es una sesión de OpenCode por proyecto, con
  historial en SQLite y una instrucción de rol: conversar, inspeccionar y proponer, **sin
  modificar archivos** ni crear tareas.
- La app tiene una pestaña **Orquestador** con el historial, envío, spinner y «Nueva
  conversación». El compositor de objetivos enlaza a ella.
- Cada respuesta del Orquestador tiene **«Crear objetivo con esta respuesta»**: abre un
  formulario editable, con la opción de planificar, y solo al pulsar «Crear objetivo» se
  llama a `/ask`. El Orquestador nunca crea ni ejecuta trabajo por sí mismo; el texto pasa
  por el planificador normal.
- `POST /ask` rechaza saludos exactos (`hola`, `buenas`, `hi`, `hello`, «qué tal», etc.) con
  `not_an_objective` antes de crear el goal. Un mensaje como «hola, arregla el login» sigue
  siendo un objetivo válido.
- El prompt del planificador ordena devolver `{"tasks":[]}` para conversaciones o preguntas,
  y un plan vacío se trata como error de planificación: el goal queda bloqueado, sin lanzar
  agentes.
- Las respuestas del chat se guardan como `user`/`assistant`; el orden usa `created_at` y
  `rowid` porque dos mensajes pueden compartir el milisegundo.

## Consecuencias

- **A favor:** preguntar ya no cuesta 4 agentes; el operador tiene un lugar para explorar
  antes de ejecutar; el guard y el prompt se refuerzan mutuamente; convertir una propuesta
  en objetivo exige una confirmación explícita y el texto se puede editar.
- **En contra:** el chat del Orquestador es una sesión distinta de la que planifica cada
  goal; su transcripción no es la del planificador. Si OpenCode reinicia la sesión, el chat
  empieza sin contexto (el historial de JCS se conserva). El consumo del chat todavía no se
  suma al total de runs.
- El planificador sigue siendo el que crea el DAG; el Orquestador no ejecuta ni aprueba
  trabajo automáticamente.

## Verificación

- `a_greeting_is_not_an_objective` (HTTP): `{"prompt":"Hola"}` devuelve 400 y no crea goal.
- `the_transcript_and_session_are_durable_and_clearable` (store): mensajes ordenados,
  sesión persistida y reset.
- Prueba real con daemon temporal y modelo gratuito: «Hola, ¿de qué trata este repositorio?»
  respondió correctamente y no creó tareas; `goals` quedó en 0.
- En el proyecto real, el goal «Hola» se canceló antes de desplegar; sus 4 tareas quedaron
  `cancelled` y los worktrees se conservaron.
