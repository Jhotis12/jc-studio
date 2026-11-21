# Pendientes y riesgos aceptados

Registro vivo de lo que **no** está hecho y de lo que se aceptó conscientemente.
Cada entrada tiene una condición de salida; sin ella, no es una decisión, es un olvido.

## Riesgos aceptados

### RUSTSEC-2026-0459 — SQLite empaquetado en `sqlx` 0.9

- **Qué es:** `sqlx` 0.9 fija `libsqlite3-sys` 0.30.1, cuyo SQLite empaquetado (< 3.54.0)
  puede devolver memoria no inicializada al abrir una base de datos junto a un `-wal`
  hermano con tamaño de página distinto.
- **Por qué se aceptó (2026-11-21):** el vector exige que la víctima abra un par base+WAL
  manipulado, y `jcsd` solo abre la base que él mismo creó, en el directorio privado del
  usuario, con permisos restringidos. No hay versión de `sqlx` que permita el paquete
  parcheado, y la alternativa (SQLite del sistema) necesita `libclang` para generar bindings.
- **Condición de salida (cualquiera de las dos):**
  1. `sqlx` pasa a permitir `libsqlite3-sys >= 0.39` → actualizar y quitar la excepción.
  2. El entorno tiene `clang`/`libclang` → cambiar el feature a `sqlite-unbundled`, verificar
     con `ldd target/debug/jcsd | grep sqlite` y quitar la excepción.
- **Dónde está anotado también:** `deny.toml` (sección `[advisories].ignore`) y ADR 0003.
- **Estado:** pendiente de revisión por el operador.

## Decisiones diferidas (no son deuda, son alcance)

| Tema | Estado | Cuándo |
|---|---|---|
| Modo `spawn` de `opencode serve` (proceso propio por instancia) | No implementado; el modo `attach` es el correcto en esta máquina | Solo si hace falta aislamiento real por proyecto (Fase 7) |
| Aprobación por *ruleset* al crear la sesión | No enviado: la precedencia de reglas no está verificada. Los packs ya declaran reglas y gates, pero no compilan todavía a `permission` de OpenCode | Fase 3, con una sonda que ejercite una herramienta real |
| Renderizar `opencode.json` y `.opencode/agents/*.md` | No implementado a propósito: el esquema de configuración de OpenCode no está verificado contra una instancia, y generar config inválida es peor que no generarla. Hoy se renderizan `AGENTS.md`, `.jcs/gates.toml` y el lock | Fase 3, con una sonda que valide el esquema |
| Merge automático de la rama de tarea | No implementado: el commit queda en su rama y el merge es manual | Fase 3 (merge queue) |
| TUI `ratatui` | No implementada: el CLI cubre el ciclo completo | Fase 3 |
| Descomposición de objetivos en DAG de tareas | No implementada: un objetivo = una tarea | Fase 3 |
| OpenSpec como flujo de cambios | No adoptado: las ADRs cubren las decisiones | Fase 2, si se quieren propuestas formales por pack |
| `sqlx-cli` y `cargo-llvm-cov` | No instalados | Fase 5 (cobertura) |

## Limitaciones conocidas

- **Backend idle sin `outcome`:** si OpenCode queda ocioso sin reportar resultado y sin
  formulario pendiente, el run se corta por presupuesto (`JCS_RUN_TIMEOUT_SECS`, 900 s por
  defecto), no de inmediato. Es lento pero nunca infinito.
- **Git vía CLI:** cada operación lanza un proceso `git`. Irrelevante frente a la latencia de
  un agente, pero es una dependencia de entorno (git ≥ 2.30).
- **Solo Unix:** el socket UDS y los permisos `0600` no existen igual en Windows.
- **Un solo daemon por base de datos:** garantizado por lockfile; no hay modo multi-usuario.
- **Gates en secuencia:** más lentos que en paralelo, pero atribuibles y deterministas.
- **Parsers:** `cargo`, `node`, `coverage` y genérico. `pytest` cae en genérico hasta que
  haya un fixture que justifique un parser propio.
