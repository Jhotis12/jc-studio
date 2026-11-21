# 0005 — Un worktree por tarea y git vía CLI

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

Varias tareas de agente en paralelo sobre el mismo repositorio se pisan: dos ramas no pueden
compartir el árbol de trabajo, y un agente que hace `git checkout` mientras otro edita produce
pérdida de trabajo. Además, la autonomía exige poder abandonar un experimento sin ensuciar
la rama principal.

## Alternativas consideradas

1. **Todo en el árbol principal, ramas secuenciales.** Simple, sin paralelismo real.
2. **Copias completas del repo por tarea.** Aislamiento máximo, coste de disco y de setup alto.
3. **`git worktree` por tarea.** Comparte object database, árbol de trabajo independiente,
   rama independiente, creación casi instantánea. Es la primitiva diseñada para esto.
4. **Solo contenedores por tarea.** Buen aislamiento de procesos, pero no resuelve el
   aislamiento de *changes* ni el merge; y es pesado para el día a día.

## Decisión

- **Un worktree por tarea**, en `<root>/.jcs/worktrees/<task-id>` (ignorado por git), rama
  `jcs/<goal-slug>/<task-slug>`, creada desde la rama base del proyecto.
- El agente de esa tarea se crea con `location.directory` apuntando a ese worktree: no puede
  tocar la copia principal.
- Al terminar: gates → commit convencional → **merge queue** (rebase sobre base + gates +
  fast-forward). Nunca `--force` contra una rama compartida.
- Worktrees se limpian al cerrar la tarea; los huérfanos se detectan con `jcs doctor`.

**Implementación de git: se invoca el binario `git`** (argumentos en array, sin shell),
no `libgit2` ni `gix`.

Motivos: paridad exacta con el comportamiento que el usuario espera (hooks, `core.autocrlf`,
firmas, `insteadOf`), cero dependencias C en el build, y los errores de git son los errores
reales de git. El coste (un proceso por operación) es irrelevante frente a la latencia de un
agente. `jcs-git` encapsula la invocación para que cambiar a `gix` sea un cambio local.

## Consecuencias

**Positivas**
- Paralelismo real sin colisiones de árbol de trabajo.
- Abandonar una tarea = borrar un worktree; la rama base queda intacta.
- El diff de una tarea es exactamente el diff de su worktree: revisable y aislado.
- Nada de force-push: el merge es fast-forward o se escala a humano.

**Negativas / costes**
- Disco: cada worktree materializa los archivos (no los objetos).
- Un worktree huérfano ocupa espacio y bloquea su rama hasta `git worktree prune`.
- Los tests que asumen rutas absolutas del repo principal fallan en worktrees; hay que
  documentarlo para los packs.
- Invocar `git` exige distinguir "no es un repo" de "git falló" en el parseo de errores.

**Reglas duras**
- Prohibido `git push --force`, `git reset --hard` sobre la rama base, y `git clean -fd`
  fuera de un worktree de tarea.
- Antes de cualquier operación destructiva: checkpoint (tag ligero `jcs/ckpt/<run-id>`).

## Verificación

- Test: dos tareas concurrentes en worktrees distintos modifican el mismo nombre de archivo
  sin conflicto entre ellas.
- Test: crear worktree, commit, merge fast-forward en la base; `git status` limpio en ambos.
- Test: worktree huérfano es detectado y reportado por `jcs doctor` (no borrado en silencio).
- Test: intento de force-push es rechazado por la capa `jcs-git` antes de ejecutarse.
