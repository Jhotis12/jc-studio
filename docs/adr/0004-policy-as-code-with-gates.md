# 0004 — Política como código con doble capa: instrucciones + gates ejecutables

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

El requisito es que el agente "sepa cómo debe hacerlo": arquitectura, patrones, convenciones,
stack, reglas de git, seguridad. La tentación es escribir un `AGENTS.md` muy largo y confiar
en que el modelo lo respete.

Eso no funciona de forma fiable. Las instrucciones en prompt son **blandas**: compiten por
atención con el resto del contexto, se diluyen en sesiones largas, se ignoran bajo presión de
la tarea y no son verificables. Un agente puede violar la arquitectura y aun así terminar con
tests verdes.

## Alternativas consideradas

1. **Solo instrucciones (prompts/skills).** Barato, flexible, no verificable. El agente
   "sabe" pero nada lo obliga.
2. **Solo gates (linters/tests).** Verificable, pero el agente no sabe qué se espera hasta
   que falla: más iteraciones, más coste, arreglos a ciegas.
3. **Instrucciones generadas + gates que las hacen cumplir.** El agente recibe el *qué* y el
   *por qué*; el gate impone el *no*.

## Decisión

La política es un artefacto versionado (**pack**) que se compila en dos capas:

**Capa 1 — Instrucciones (blanda, para el agente):**
`AGENTS.md` del proyecto, `.opencode/agents/*.md`, skills, y reglas de permisos en
`opencode.json`. Se generan, no se editan a mano.

**Capa 2 — Gates (dura, para la realidad):**
comandos ejecutables con veredicto tipado: build, lint, typecheck, tests, cobertura,
`cargo-deny`/`npm audit`, escaneo de secretos, límites de diff y **fitness functions
arquitectónicas** (p. ej. "el módulo `domain` no importa `infra`").

Composición determinista con precedencia explícita:

```
core (inmutable) < pack < proyecto < overrides locales
```

Resolución de dependencias entre packs (`requires`), detección de conflictos en la misma
clave, y **lockfile con hash por archivo y por pack** — mismo patrón que `skills-lock.json`
ya presente en este repo (`commit` + `computedHash`).

Drift es un error, no una molestia: `jcs policy render --check` falla si el árbol generado
no coincide byte a byte con lo que produciría la política actual. Ese comando es un gate de CI.

## Consecuencias

**Positivas**
- "Cómo debe hacerlo" deja de ser una promesa y pasa a ser un contrato verificable.
- Cambiar una regla y ver el efecto es reproducible: mismo pack + mismo proyecto = mismo render.
- Los packs son portables entre proyectos y auditables (hash + versión + origen).

**Negativas / costes**
- Escribir y mantener gates buenos es trabajo real (y el trabajo que de verdad importa).
- Falsos positivos de gates erosionan la confianza: cada gate nuevo necesita justificación y
  una ruta de escape auditada (`allow` con motivo y caducidad, registrado en el event log).
- Un render determinista exige ordenar claves y normalizar finales de línea.

**Riesgos mitigados**
- Gate que siempre pasa (teatro): los gates de política se auto-verifican con un test de
  mutación — se introduce una violación conocida y el gate **debe** fallar.
- Deriva entre render y edición manual: `--check` en CI + aviso en la TUI.

## Verificación

- `jcs policy render` dos veces produce árbol idéntico (test de determinismo).
- `jcs policy render --check` con un archivo editado a mano → exit != 0.
- Test de mutación: fixture que viola una regla de boundaries → el gate falla con `file:line`.
- Cambiar una regla del pack y re-renderizar cambia el comportamiento del agente en un test
  de regresión de política (el prompt enviado contiene la regla nueva).
