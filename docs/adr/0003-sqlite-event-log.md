# 0003 — SQLite + sqlx con event log append-only

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

El daemon necesita estado durable con garantías de crash-recovery: si el proceso muere a
mitad de una tarea, al reiniciar debe saber exactamente qué pasó, qué está en vuelo y qué
quedó a medias. También necesita consultas (lista de tareas, historial, coste) y un stream
de eventos para clientes.

Restricciones reales: una sola máquina, un solo usuario, sin servidor de base de datos, y
la TUI necesita latencias de milisegundos.

## Alternativas consideradas

1. **PostgreSQL.** Excelente para concurrencia multi-escritor y equipos. Pero exige un
   servicio externo, credenciales y migraciones coordinadas para un producto de escritorio.
   Sobredimensionado para un daemon de usuario.
2. **Archivos JSON/TOML.** Sin garantías transaccionales ni consultas. Descartado.
3. **SQLite embebido, un archivo, WAL.** ACID, cero administración, lecturas concurrentes,
   backup trivial (copiar archivo). Un solo escritor, que es exactamente nuestro modelo
   (el daemon es el único dueño).
4. **DuckDB / embedded columnar.** Bueno para analítica, malo para OLTP con actualizaciones
   pequeñas y frecuentes.

## Decisión

- **SQLite** como almacén, en `$XDG_DATA_HOME/jcs/jcs.db`.
- **`sqlx`** con `SqlitePool`, *runtime queries* (no macros `query!` que exijan base de datos
  en tiempo de compilación, para no romper builds limpios).
- **WAL**, `busy_timeout = 5s`, `foreign_keys = ON`, `synchronous = NORMAL`.
- **Un lockfile exclusivo** junto a la base para impedir dos daemons.
- **Event log append-only** como fuente de verdad de lo ocurrido: tabla `events(seq INTEGER
  PRIMARY KEY AUTOINCREMENT, id, ts, kind, project_id, task_id, run_id, payload JSON)`.
  Las tablas de estado (`goals`, `tasks`, `runs`) son **proyecciones** para consulta rápida.
- Toda mutación de estado se hace **en la misma transacción** que el append del evento.
- Las migraciones se embeben con `sqlx::migrate!` y se aplican al arrancar.

## Consecuencias

**Positivas**
- Crash recovery basado en hechos registrados, no en suposiciones.
- El stream a clientes es `SELECT ... WHERE seq > ?`: reconexión y *catch-up* gratis,
  sin bus de mensajes externo.
- Backup = copia de un archivo (con `VACUUM INTO`).
- Auditoría completa de acciones humanas y automáticas.

**Negativas / costes**
- Un solo escritor: si en el futuro hay múltiples daemons o multi-usuario, hay que migrar.
  El esquema y la capa `jcs-store` aíslan esa decisión.
- El log crece sin límite: se necesita compactación/archivado (política definida abajo).
- `AUTOINCREMENT` da `seq` monótono pero con huecos posibles en rollback: los clientes
  deben tolerar huecos y usar `since` como cota, no como conteo exacto.

**Política de retención**
- Eventos `session.text.delta` y similares (alta frecuencia, bajo valor histórico) se
  agregan por paso antes de persistir.
- Eventos de alta frecuencia no se guardan por token: se guarda inicio/fin y el resumen.
- Compactación mensual a tabla `events_archive` para todo evento de más de 90 días que no
  sea `run.*`, `gate.*`, `commit.*` ni `approval.*` (esos son permanentes por auditoría).

## Enmienda (2026-11-21): arranque sobre estado sucio

Dos reglas que solo aparecieron al matar el daemon con `kill -9`:

1. **Las tareas en vuelo se aparcan como `Blocked`, nunca como `Failed`.** Un proceso muerto
   no demuestra nada sobre el trabajo. `Running` y `Verifying` pueden ir directos a `Blocked`;
   pasar por `Failed` es una transición ilegal y, en la primera versión, **impidió arrancar**.
2. **La recuperación no puede impedir el arranque.** Si falla, se registra el error y el daemon
   arranca igual: negarse a bootear deja al operador sin forma de inspeccionar el estado que
   necesita recuperar.

## Verificación

- Test: `kill -9` simulado entre transacciones → al reabrir, `PRAGMA integrity_check` = `ok`
  y el log es consistente (ningún evento apunta a una fila inexistente).
- Test: aplicar migraciones dos veces es idempotente.
- Test: dos `Store::open` sobre el mismo archivo → el segundo falla por lockfile.
- Test: `events_since(seq)` devuelve estrictamente creciente y nunca repite `seq`.
