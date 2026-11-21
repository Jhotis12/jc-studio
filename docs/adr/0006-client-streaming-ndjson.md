# 0006 — TUI ratatui y streaming NDJSON sobre el socket (no SSE para el cliente)

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

La TUI necesita: stream continuo de eventos (texto del agente, llamadas a herramientas,
resultados de gates), reconexión sin perder eventos, y latencia baja con volúmenes altos
(un agente puede emitir cientos de deltas por segundo).

El daemon ya consume **SSE** de OpenCode. La tentación es reexponer SSE hacia el cliente.

## Alternativas consideradas

1. **Reexponer SSE por el UDS.** Reutiliza el formato, pero exige parser SSE incremental en
   el cliente y —más importante— SSE está diseñado para HTTP a través de proxies: el
   `Last-Event-ID` vive en la cabecera, la reconexión es del navegador, y el framing añade
   `data:`/`event:` por línea sin aportar nada sobre un socket local.
2. **WebSocket.** Bidireccional y estándar, pero añade handshake, framing y una dependencia
   más; nuestro tráfico de eventos es unidireccional.
3. **NDJSON: un JSON por línea sobre `GET /events?since=N`.** Trivial de parsear en streaming,
   reconexión = repetir la petición con `since` = último `seq` recibido, y el `seq` va **dentro**
   del payload, así que no depende de cabeceras. Compatible con `curl` y con `jq` para depurar.
4. **Polling periódico.** Simple pero introduce latencia y carga; malo para streaming de tokens.

## Decisión

- **Transporte:** HTTP/1.1 sobre Unix domain socket, implementado con **`hyper`** en ambos
  lados. `reqwest` se reserva para el tráfico TCP loopback hacia OpenCode.
- **Stream de eventos:** `GET /events?since=<seq>` con
  `Content-Type: application/x-ndjson`; una línea = un evento completo serializado.
  El cliente reconecta con `since = último seq visto`; el servidor nunca envía eventos con
  `seq <= since`.
- **Comandos:** `POST` con JSON y respuesta JSON (sin streaming salvo `ask --follow`).
- **TUI:** `ratatui` + `crossterm`. Modelo de UI = estado reducido desde eventos; el render
  nunca hace I/O. Buffers circulares acotados por panel y *spill* a disco para sesiones largas.
- **Backpressure:** si el cliente no lee, el daemon corta el stream con un evento `lagged`
  (con el `seq` desde el que reanudar) en vez de acumular memoria sin límite.

## Consecuencias

**Positivas**
- Parser de líneas en vez de máquina de estados SSE en el cliente: menos código, menos bugs.
- Reanudación explícita y comprobable (`since` es un parámetro, no una cabecera implícita).
- Debug con `curl --unix-socket ... /events?since=0 | jq` sin herramientas especiales.
- `hyper` sobre UDS es ~60 líneas por lado y no añade dependencias nuevas.

**Negativas / costes**
- Dos formatos de stream en el sistema (SSE hacia OpenCode, NDJSON hacia el cliente):
  hay que documentar cuál se usa dónde (queda en `docs/contracts/CONTRACT.md`).
- Sin backpressure real de TCP: hay que implementarla a mano con límites y `lagged`.
- `hyper` es de más bajo nivel que `reqwest`: más código de cliente que mantener.

**Nota de compatibilidad**
- El `seq` es monótono pero puede tener huecos (rollbacks); el cliente no debe asumir
  contigüidad, solo orden creciente.

## Verificación

- Test: cliente que se desconecta a mitad de stream y reconecta con `since` recibe
  exactamente los eventos posteriores, sin duplicados ni omisiones.
- Test: productor rápido + consumidor lento → se emite `lagged` y no crece la memoria
  del daemon (medido con un contador de eventos en vuelo).
- Test: `GET /events?since=0` en un daemon recién arrancado devuelve el log completo en orden.
- Benchmark: 10 000 eventos/segundo sobre UDS sin perder líneas ni bloquear el runtime.
