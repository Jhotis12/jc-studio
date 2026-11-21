# Contrato con OpenCode

Versión soportada: **2.3.2**. Copia del documento OpenAPI completo:
[`opencode-2.3.2.json`](opencode-2.3.2.json) (142 paths, 173 operaciones).

Este archivo documenta **solo lo que usamos y cómo se comporta de verdad** — no lo que el
schema sugiere. Todo lo marcado como *verificado* se comprobó contra una instancia viva.

Regenerar la copia:

```bash
opencode api GET /openapi.json > docs/contracts/opencode-2.3.2.json
python3 scripts/check_contract.py
```

## Descubrimiento del servicio

| Qué | Cómo |
|---|---|
| URL | `opencode service status` imprime la URL base (puerto dinámico) |
| Credencial | `~/.config/opencode/service.json`, campo `password` (modo 0600) |
| Overrides | `JCS_OPENCODE_URL`, `JCS_OPENCODE_PASSWORD` |
| Auth | HTTP Basic, usuario `opencode`, contraseña de instancia |

**Verificado:** usuario vacío o sin cabecera → `401 Unauthorized`. `opencode:<password>` → `200`.
`opencode serve --port N` imprime `server password <pw>` al arrancar, así que el modo *spawn*
también puede descubrir la credencial sin adivinar.

## Formas de respuesta (inconsistentes, a propósito)

| Endpoint | Envoltura |
|---|---|
| `GET /api/info` | objeto plano |
| `GET /api/project` | **array plano** |
| `GET /api/model` | `{"data": [...]}` |
| `POST /api/session` | `{"data": {...}}` |
| `GET /api/session/{id}/diff` | `{"data": [...]}` |
| `GET /api/session/{id}/message` | `{"data": [...], "cursor": {...}}` |
| Errores | `{"_tag": "...", "message": "..."}` |

Por eso el cliente desenvuelve `data` **solo** cuando es la única clave del objeto: una
entidad que legítimamente tenga un campo `data` no debe aplanarse.

## Stream de eventos: `GET /api/event`

**Verificado contra una sesión real.** Dos detalles que no se deducen del schema:

1. **No se usa el campo SSE `event:`.** Todo llega como líneas `data:` cuyo JSON lleva el
   tipo en `"type"`. Un parser SSE genérico despacha el evento equivocado.
2. **Hay comentarios `: heartbeat`** que deben ignorarse sin cerrar el stream.

Forma de cada evento:

```json
{"id":"evt_...","created":1795274772700,"type":"session.execution.succeeded",
 "data":{"sessionID":"ses_..."},"durable":{"aggregateID":"...","seq":0,"version":1}}
```

### Ciclo de vida observado (modelo gratuito, ~5 s)

```
session.created
session.inbox.enqueued → session.execution.started → session.inbox.delivered
session.step.started → session.step.first-token
session.text.started → session.text.delta* → session.text.ended
session.step.streamed → session.step.ended → session.usage.updated
session.execution.succeeded
```

Payloads que consumimos:

| Tipo | `data` |
|---|---|
| `session.execution.*` | `{sessionID}` (`succeeded`/`failed`/`interrupted`) |
| `session.usage.updated` | `{sessionID, cost, tokens:{input,output,reasoning,cache:{read,write}}}` |
| `session.text.delta` | `{sessionID, assistantMessageID, ordinal, delta}` |
| `session.step.ended` | `{..., tokens, cost, files[]}` |
| `session.tool.*` | forma **no verificada** (la sonda no ejecutó herramientas) |

Consecuencias de diseño:

- `cost` y `tokens` son **acumulados de la sesión**, no deltas: se reemplazan, nunca se suman.
- El evento terminal es `session.execution.*`; `session.idle` es señal secundaria.
- Como el stream es global, cada evento se filtra por `sessionID`. El filtro `?types=` del
  servidor **existe pero su formato de lista no está verificado** (el schema declara un
  `string` con `enum` de un solo valor, lo que no expresa una lista). No se usa.
- Los eventos de herramientas se leen de forma defensiva (`tool`/`name`, `input`/`args`)
  precisamente porque no están verificados.

## Permisos

- `GET /api/session/{id}/permission` → solicitudes pendientes.
- `POST /api/session/{id}/permission/{requestID}/reply` con
  `{"decision":"once"|"always"|"reject","message":"..."}`.
- `Permission.Rule` = `{action, resource, effect}` con `effect ∈ {allow, deny, ask}`.

**No enviamos un ruleset al crear la sesión**: la precedencia entre reglas y el significado
exacto de `action`/`resource` no están verificados. Adivinar un control de seguridad es peor
que no tenerlo. La aplicación se hace respondiendo solicitudes reales y **sondeando**
`/permission` cada 2 s, para no bloquear la sesión si cambia la forma del evento.

El ruleset por política es trabajo de la Fase 2, y requiere una sonda con una herramienta real.

## Endpoints usados

| Operación | Uso |
|---|---|
| `server.info` | health y versión |
| `project.list` | proyectos conocidos por el backend |
| `model.list` | validar referencias `provider/model` |
| `session.create` | sesión con `location.directory` = worktree de la tarea |
| `session.prompt` | objetivo inicial (`delivery: queue`) |
| `session.get` | `outcome` autoritativo para reconciliación |
| `session.message.list` | historial para reconstruir el transcript |
| `session.diff` | cambios por turno |
| `session.interrupt` | aborto |
| `session.permission.list` / `reply` | aprobación |
| `event.subscribe` | stream en vivo |

`session.create` acepta `location.directory`, y `project.list` devuelve varios proyectos:
**un solo servidor sirve a todos los proyectos**. Eso es la base del modo *attach* (ADR 0002).

## Al actualizar OpenCode

1. Actualizar el binario.
2. Regenerar `opencode-2.3.2.json` (renombrar con la versión nueva).
3. `python3 scripts/check_contract.py` — falla si desaparece una operación usada.
4. Ejecutar el E2E opt-in (`JCS_E2E=1 cargo test`) y revisar que las formas de evento
   siguen siendo las documentadas aquí.
5. Actualizar este archivo con lo que haya cambiado.
