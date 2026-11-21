# 0007 — Packs en TOML, composición determinista y bucle de reparación

- **Estado:** Accepted
- **Fecha:** 2026-11-21
- **Complementa a:** [0004](0004-policy-as-code-with-gates.md)

## Contexto

ADR 0004 fijó que la política tiene dos capas: instrucciones y gates ejecutables. Faltaba
decidir **cómo se escribe, se compone y se verifica** esa política.

Requisitos que condicionan el diseño:

- La política vive en el repositorio del proyecto y se revisa en un PR.
- Dos ejecuciones con los mismos packs deben producir **los mismos bytes**, o `--check` no
  sirve como gate.
- El proyecto debe poder ajustar reglas sin forkear un pack.
- Un pack de terceros no puede ejecutar código arbitrario por la puerta de atrás.
- Un agente no puede arreglar un gate fallido si el mensaje no le dice qué arreglar.

## Alternativas consideradas

1. **YAML.** Es lo que pedía el plan original. Pero el ecosistema `serde_yaml` está sin
   mantenimiento, y las alternativas mantenidas son forks o parsers nuevos. Añadir una
   dependencia no mantenida a un sistema cuya propuesta es la durabilidad es incoherente
   con la lección de `chrono`.
2. **JSON.** Sin comentarios y hostil para escribir a mano.
3. **TOML + Markdown.** Mantenido, sin ambigüedad de indentación, con errores de parseo
   precisos, y ya es el formato nativo del workspace (`Cargo.toml`, `justfile`).
4. **Lenguaje embebido (Starlark, Lua).** Potentísimo y también una superficie de ejecución
   arbitraria que no queremos en v1.

## Decisión

**Formato.** Cada pack es un directorio con `pack.toml` y documentos Markdown. El manifiesto
declara `id`, `version`, `requires`, `rules`, `gates`, `skills` e `instructions`. Los campos
desconocidos se **rechazan**: un `timeout_secs` mal escrito no debe convertirse en un default
silencioso de una hora.

**Ubicación.** Los seis packs base van **embebidos en el binario** (`include_str!`), para que
`jcs policy` funcione sin instalar nada. `JCS_PACKS_DIR` añade o sustituye packs por `id`.

**Precedencia.** `requires` (dependencias primero) → orden de selección → overrides del
proyecto. En conflicto gana el último, y cada regla y cada gate guardan su origen para que
`jcs policy show` responda "¿quién decidió esto?".

**Proyecto.** `.jcs/policy.toml` en el repositorio, versionado con el código: packs, reglas
propias y listas de desactivación. Si no existe, se usan `JCS_DEFAULT_PACKS`; si tampoco hay,
no hay política y solo corren los gates de commit integrados.

**Integridad y drift.** `.jcs/policy.lock` fija el hash de cada pack y de cada archivo
renderizado. `jcs policy render --check` recalcula en memoria y compara **bytes**: cualquier
edición manual o pack cambiado sin re-renderizar es un fallo, no un aviso.

**Gates.** El comando es un **array de argv**, nunca una cadena de shell: un pack no puede
colar comillas ni expansión. Un pack que de verdad necesite una tubería debe escribir
`["sh", "-c", "..."]`, donde un revisor lo ve. Timeout con muerte del **grupo de procesos**,
salida acotada por stream y parsers tipados con `findings` que incluyen `archivo:línea`.

**Bucle de reparación.** Si un gate requerido falla, el executor devuelve al agente la salida
recortada y accionable, hasta `JCS_MAX_REPAIR_CYCLES` (2 por defecto). El prompt prohíbe
explícitamente debilitar tests o tocar la configuración de gates. Al agotar el presupuesto, la
tarea pasa a `Blocked` con los gates que fallan, no a `Failed`: el trabajo sigue siendo válido,
lo que falta es una decisión.

## Consecuencias

**Positivas**
- La política es revisable en un PR y verificable con un comando.
- Un pack no puede escalar privilegios por interpolación de shell.
- El drift es imposible de ignorar sin romper CI.
- El agente recibe el *qué* incumplió y el *dónde*, no un "algo falló".

**Negativas / costes**
- TOML es más verboso que YAML para documentos largos; por eso el texto va en Markdown.
- Los packs embebidos requieren recompilar para cambiarlos (mitigado por `JCS_PACKS_DIR`).
- Los gates corren en secuencia: más lento que en paralelo, pero atribuible y determinista.
- Un gate con `sh -c` sigue siendo código; la revisión del pack es la que lo controla.

**Riesgos mitigados**
- Teatro de calidad: un gate que siempre pasa se detecta con un test que introduce una
  violación conocida y exige que falle.
- Pérdida de instrucciones propias: un `AGENTS.md` escrito a mano **no** se sobrescribe sin
  `--force`, y al forzarlo se guarda `AGENTS.md.pre-jcs.bak`.

## Verificación

- `crates/jcs-policy`: determinismo (dos renders idénticos), precedencia, desactivaciones,
  ciclos, hash del lock, rechazo de campos desconocidos, escape de rutas.
- `crates/jcs-gates`: timeout mata el grupo de procesos, salida acotada, cobertura que
  falla con exit 0, gate inexistente es `Errored`.
- `scripts/e2e-policy.sh`: un gate exige un token que el objetivo no menciona; el run solo
  puede terminar en verde si el bucle de reparación se ejecutó de verdad.
