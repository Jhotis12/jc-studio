# 0017 — El gate de secretos mira solo lo que la tarea escribió

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

Dos tareas seguidas de un objetivo real entraron en un bucle de reparación por el gate
`secrets`. El escáner recorría **todo** el worktree y marcaba como credenciales:

- `OPENCODE_EXPERIMENTAL_BASH_DEFAULT_TIMEOUT_MS=1800000` y `EMBEDDINGS_MODEL=…` en
  `.env.example` (nombres largos con guiones bajos);
- `speaches-ai/Kokoro-82M-v1.0-ONNX` en tests y documentación (nombre de modelo);
- hashes base64 de `bun.lock` y del HTML generado por `graphify`.

El agente no podía arreglar esos falsos positivos, así que empezó a modificar archivos que
no tenían nada que ver con su tarea, a cambiar literales por constantes y a estudiar cómo
evadir el escáner (padding, NUL, comentarios enormes). Uno de los dos runs terminó con
OpenCode sin responder tras diez minutos de razonamiento.

## Alternativas consideradas

1. **Allowlist por proyecto.** Traslada el problema al operador: cada modelo, hash o variable
   nueva exigiría editar la política.
2. **Bajar el umbral de entropía o ignorar mayúsculas.** Deja de detectar credenciales
   reales; no es aceptable para un gate de seguridad.
3. **Escanear solo las líneas añadidas por la tarea.** El gate conserva su función —impedir
   que *este* cambio introduzca una credencial— y deja de juzgar el repositorio heredado.

## Decisión

- `jcs-git::Repo::working_changed_lines()` devuelve las líneas añadidas del diff contra
  `HEAD` (staged y no staged) y el contenido completo de los archivos no rastreados. Los
  archivos ignorados por git no se incluyen.
- `verify.rs` pasa esas líneas a `jcs_gates::scan_targets`. Ya no se recorre el árbol.
- El escáner omite lockfiles (`bun.lock`, `Cargo.lock`, `package-lock.json`, …), salidas
  generadas (`graphify-out/`, `dist/`, `build/`, `target/`) y minificados.
- La heurística de entropía ignora tokens con separadores de ruta/modelo/versión
  (`/ - _ .`). Las credenciales con prefijo conocido (`AKIA`, `ghp_`, `sk-proj-`, …) se
  siguen detectando antes de esa heurística.
- El mismo criterio se aplica en el rebase de integración: si el código cambió bajo la
  tarea, se vuelve a escanear solo lo que la tarea añadió.

## Consecuencias

- **A favor:** un cambio limpio no falla por cosas que ya estaban en el repositorio; el
  agente deja de tocar archivos ajenos para contentar al gate; el coste por tarea baja.
- **En contra:** una credencial preexistente que nadie toca no se reporta en ese run. Para
  auditar el repositorio completo sigue existiendo `scan_secrets(root, options)`, que recorre
  el árbol con las mismas omisiones. El gate de tarea no sustituye a una auditoría de
  secretos del proyecto.
- Si un agente hace commits propios antes de la verificación, el diff contra `HEAD` no ve
  esos cambios. El contrato del ejecutor es que el agente no commitea; el commit lo hace el
  daemon después de los gates.

## Verificación

- Tests: línea añadida con clave de alta entropía detectada; nombre de modelo, variable de
  entorno y lockfile ignorados; archivo no rastreado escaneado completo; diff con prefijos
  `c/`/`w/` (configuración local de git) resuelto a rutas relativas.
- E2E real con `deepseek-v4.1-flash` en un repo temporal que contenía el nombre de modelo y
  el hash en `bun.lock`: la tarea creó `NOTES.md`, el gate `secrets` pasó y el goal terminó
  `succeeded`.
