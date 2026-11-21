# 0001 — Daemon `jcsd` + cliente fino por Unix socket

- **Estado:** Accepted
- **Fecha:** 2026-11-21
- **Decisores:** operador (jhotis)

## Contexto

El producto tiene que sobrevivir a que se cierre la terminal, aceptar comandos remotos
(Telegram), ejecutar trabajo largo (agentes, gates, tests) y exponer el mismo estado a
varios consumidores. A la vez, la experiencia principal es una TUI de consola.

Un binario monolítico que contenga la TUI y el orquestador hace imposible lo anterior:
si la TUI muere, muere el trabajo; si el bot escribe estado y la TUI también, hay dos
escritores sobre el mismo estado; y no se pueden abrir dos terminales sobre la misma sesión.

## Alternativas consideradas

1. **Monolito TUI.** Menos procesos, pero estado acoplado al ciclo de vida de la terminal.
   Descartado: incumple "cerrar la laptop y que el trabajo continúe".
2. **Daemon + API TCP en loopback.** Simple de consumir, pero cualquier proceso del usuario
   (o cualquier usuario de la máquina, según configuración) puede hablar con el daemon.
3. **Daemon + Unix domain socket con permisos 0600.** Aislamiento por permisos del sistema
   de archivos, sin puerto abierto, sin TLS que gestionar.
4. **Cliente/servidor con base de datos compartida (sin RPC).** Varios procesos escribiendo
   SQLite directamente. Descartado: contención de locks, migraciones concurrentes, sin
   punto único donde aplicar política y auditoría.

## Decisión

Un **daemon de usuario** (`jcsd`) es el único dueño del estado durable, del orquestador, de
los gates, del pool de agentes, de git y de las integraciones (Telegram). Expone una API HTTP
sobre **Unix domain socket** en `$XDG_RUNTIME_DIR/jcs/jcsd.sock` con modo `0600`.

Los clientes (`jcs` TUI/CLI, el bot de Telegram, futuros consumidores) son **finos**: no
tienen lógica de negocio, solo presentan estado y envían comandos. Si un cliente muere, el
trabajo continúa.

El daemon corre bajo el gestor de servicios del usuario (systemd `--user`) y hace
*graceful shutdown*: deja de aceptar conexiones, marca runs en vuelo como `interrupted`
(recuperables), libera leases y cierra SQLite.

## Consecuencias

**Positivas**
- Trabajo durable, multi-cliente, multi-terminal, sin estado en el cliente.
- Un único punto donde aplicar autorización, auditoría y rate limiting.
- El bot remoto no necesita lógica propia: es otro cliente.

**Negativas / costes**
- Hay que versionar el protocolo cliente↔daemon (`jcs-proto`).
- Depurar requiere mirar logs del daemon, no solo stdout.
- En Windows no hay UDS clásico: requerirá named pipes (fuera de alcance v1).

**Riesgos mitigados**
- Socket huérfano tras crash: al arrancar, si el path existe y nadie responde a `health`,
  se elimina y se re-crea (nunca se borra un socket vivo).
- Dos daemons sobre el mismo estado: se toma un *lockfile* exclusivo junto a la base de datos.

## Verificación

- `jcs doctor` falla si el socket existe con permisos distintos de `0600`.
- Test de integración: matar el cliente a mitad de un `ask` no cancela el run en el daemon.
- Test de caos: `kill -9` al daemon y reinicio → no hay runs duplicados ni commits duplicados.
