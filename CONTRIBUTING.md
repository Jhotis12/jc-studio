# Contribuir a JC Studio

## Idioma

- **Código, identificadores, comentarios y mensajes de commit:** inglés.
- **Documentación de producto y ADRs:** español.

Motivo: los identificadores en inglés son portables y encajan con el ecosistema Rust;
las decisiones de producto las lee el operador y deben estar en su idioma.

## Flujo de trabajo

1. **Decisión primero.** Si el cambio toca arquitectura, durabilidad, seguridad o el contrato
   con OpenCode, escribe o actualiza una ADR en `docs/adr/` **en el mismo PR**.
2. **Test primero.** Un bug se reproduce con un test que falla antes de arreglarlo.
3. **Cambios pequeños.** Un commit = una unidad coherente y reversible.
4. **Gates verdes.** `cargo fmt`, `clippy -D warnings`, tests, y `cargo-deny`.

## Mensajes de commit (Conventional Commits)

```
<type>(<scope>): <descripción en imperativo, minúscula, sin punto final>

<cuerpo opcional: por qué, no qué>

<footer opcional: BREAKING CHANGE, Refs, Co-authored-by>
```

Tipos: `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `perf`, `ci`.
Scopes habituales: `core`, `proto`, `store`, `agent`, `git`, `daemon`, `cli`, `policy`, `gates`.

Ejemplos:

```
feat(store): append-only event log with transactional state projection
fix(agent): reconnect sse stream with last-event-id after idle timeout
docs(adr): adopt attach mode for the opencode backend
```

Un commit no debe mezclar refactor y cambio de comportamiento: eso hace el revert imposible.

## Convenciones de código

- **Sin `unwrap()`/`expect()` fuera de tests.** Errores con `thiserror` en librerías y
  `anyhow` en binarios.
- **Sin `unsafe`** salvo justificación explícita en comentario y revisión.
- **Sin `println!` fuera de `jcs-cli`**: usa `tracing`.
- **Inmutabilidad por defecto** en estructuras de dominio.
- **Archivos pequeños y cohesivos.** Si un archivo pasa de ~800 líneas, es una señal.
- **Comentarios que explican *por qué*, no *qué*.** El *qué* lo dice el código.
- Errores de usuario con contexto accionable: qué falló, dónde y qué hacer.

## Tests

- Unitarios: dentro del crate, en `#[cfg(test)]`.
- Integración: `tests/` por crate, con `tempfile` para estado en disco.
- Property tests con `proptest` para máquinas de estados.
- **Socket real**: `crates/jcs-daemon/tests/socket.rs` arranca el binario `jcsd` y habla con él
  por el UDS (exclusividad, socket obsoleto, permisos). No necesita modelo ni OpenCode.
- **End-to-end real**: `just e2e` — objetivo → worktree → agente → gates → commit, con
  aserciones sobre el contenido del commit. Requiere `opencode service status` y usa un
  modelo gratuito por defecto (`JCS_E2E_MODEL` para cambiarlo).
- **Durabilidad**: `just chaos` — `kill -9` a mitad de run, reinicio, y verificación de
  `interrupted` + `blocked`, cero commits parciales y un run posterior correcto.

Los dos últimos son la evidencia de las afirmaciones del README; si fallan, el producto no
está terminado.

## Seguridad

- Nunca loguees credenciales, tokens ni contenido de `~/.config/opencode/service.json`.
- Las rutas que vienen del cliente se validan y se normalizan (`canonicalize`) antes de usarse.
- Todo comando externo se ejecuta con **argv separado**, nunca con `sh -c` interpolado.
- Antes de mergear: `cargo deny check` y revisión de secretos.

## Checklist de PR

- [ ] Tests nuevos o actualizados, y pasan.
- [ ] `cargo fmt --all -- --check` limpio.
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` limpio.
- [ ] ADR actualizada si cambió una decisión cara de revertir.
- [ ] Sin secretos, sin `dbg!`, sin código comentado.
- [ ] El mensaje de commit explica el *por qué*.
