# 0016 — Markdown en las respuestas y streaming real del Orquestador

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

Las respuestas de los agentes se mostraban como Markdown literal (`**negrita**`,
`` `código` ``, bloques) y el chat del Orquestador esperaba la respuesta completa antes de
pintarla. Las tareas ya emitían `agent_text` cada 300 ms, pero el Orquestador hacía un POST
bloqueante y devolvía todo al final.

## Alternativas consideradas

1. **`egui_commonmark`.** Cómodo, pero sus versiones siguen a egui y el cliente está anclado
   a 0.31; mezclar dos egui o actualizar la app por un renderizador era peor que el problema.
2. **Markdown literal.** Es lo que había; el operador lo rechazó.
3. **Renderizador local + streaming por el event log.** Sin dependencias nuevas, reutiliza
   el canal NDJSON que ya usa la app y no duplica transporte.

## Decisión

- `ui/markdown.rs` renderiza encabezados, párrafos, listas, citas, reglas, tablas, bloques
  de código e inline (negrita, cursiva, código, enlaces). Los enlaces se muestran como
  `texto (url)` sin abrir navegador. Los globs como `*.rs` no se interpretan como cursiva.
- El Orquestador ya no responde con el transcript completo:
  - `POST /orchestrator` valida, guarda el mensaje del usuario, adquiere el turno y **responde
    inmediatamente**.
  - Un task de fondo se suscribe al stream de OpenCode antes del prompt, guarda
    `orchestrator_text` y `orchestrator_reasoning` en el event log cada ~250 ms y, al acabar,
    persiste el mensaje final y emite `orchestrator_finished`.
  - La app consume esos eventos en la misma conexión NDJSON que ya tenía y pinta el texto y
    el pensamiento en vivo. El mensaje final queda en `orchestrator_messages` para recargas.
  - Un guard serializa los turnos; «Nueva conversación» espera a que termine el actual.
- El chat de cada tarea ya usaba `agent_text` cada 300 ms; ahora el mismo renderizador
  Markdown pinta sus respuestas.

## Consecuencias

- **A favor:** las respuestas se leen; el Orquestador muestra actividad desde el primer
  delta; no hay una dependencia frágil de UI.
- **En contra:** el streaming depende del proveedor: el modelo gratuito probado entregó el
  texto casi entero al final (12,6 s), mientras que `deepseek-v4.1-flash` envió razonamiento a
  los 2,5 s y texto desde los 6,1 s. El renderizador no resalta sintaxis, no abre enlaces y
  las tablas son básicas. El razonamiento en vivo no se persiste en la conversación (solo el
  texto final); el coste del chat sigue fuera del total.
- Si el backend no emite deltas, la app muestra el spinner y el pensamiento hasta que llegue
  la respuesta; no se inventa un efecto de máquina de escribir.

## Verificación

- `cargo test -p jcs-app markdown`: encabezados, listas, enlaces y separadores de tabla.
- Prueba real con `deepseek-v4.1-flash`: `POST` en 10 ms, primer razonamiento a 2,5 s, primer
  texto a 6,1 s, fin a 6,7 s, 4 chunks de texto y 2 de razonamiento.
- Prueba con el gratuito: el POST volvió en 10 ms, pero el texto llegó a los 12,6 s; se
  documenta como limitación del modelo, no del transporte.
- Inspección visual: negritas/cursivas/código renderizados y botón «Crear objetivo con esta
  respuesta» en el mensaje.
