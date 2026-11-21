# 0009 — Memoria de código: grafo, búsqueda híbrida y contexto citado

- **Estado:** Accepted
- **Fecha:** 2026-11-21
- **Complementa a:** [0008](0008-durable-orchestrator.md)

## Contexto

El agente trabaja dentro de un worktree, pero no "ve" el repositorio: explora leyendo
archivos, y cada lectura cuesta tokens y tiempo. La ventana de contexto es finita y el
código de un proyecto real no cabe.

"RAG" es fácil de prometer y difícil de verificar. Sin una métrica de recuperación, una capa
de memoria es una caja negra que a veces acierta. Y sin citas, el agente no puede decir de
dónde salió una afirmación.

## Alternativas consideradas

**Estructura**
1. *Solo embeddings sobre ventanas de texto.* Barato de construir, malo para "¿dónde está
   definido X?": el embedding de un fragmento no dice qué símbolo contiene.
2. *Solo LSP.* Preciso para un lenguaje a la vez, exige un servidor por lenguaje y no cubre
   SQL ni documentación.
3. *tree-sitter + chunking por símbolo.* Multi-lenguaje, sin servidores, y los chunks
   coinciden con unidades que un humano pegaría en un prompt.

**Semántica**
4. *Embeddings obligatorios.* Convierte una dependencia externa en un requisito de arranque.
5. *Extensión vectorial (`sqlite-vec`).* Mejor a escala, pero exige cargar una extensión
   dinámica en SQLite; complejidad de build y despliegue para un beneficio que aún no se
   necesita.
6. *Embeddings opcionales + coseno acotado en Rust.* Sin dependencias nuevas, con un límite
   explícito y medible.

## Decisión

**Estructura.** tree-sitter para Rust, TypeScript/TSX, JavaScript y Python; SQL se escanea
por líneas porque el DDL es regular y una gramática no aportaría información. Se extraen
símbolos (`function`, `struct`, `class`, `table`, `test`…) y aristas (`import`, `call`,
`references`).

**Chunks.** Alineados a símbolos, con un chunk de cabecera para los comentarios de módulo
que ningún símbolo cubre. El chunk es la unidad de recuperación y de cita.

**Búsqueda híbrida.** Tres señales que fallan distinto:
- **FTS5** (BM25) para las palabras que el usuario escribió, incluidos comentarios;
- **embeddings** (opcionales) para la idea cuando las palabras difieren;
- **grafo** para el código que llama al código encontrado.

Se combinan con `max` por chunk y un pequeño bonus cuando varias señales coinciden. Además,
una coincidencia exacta de símbolo tiene prioridad: `Store` debe devolver la definición, no
cada uso de la palabra.

**Embeddings opcionales.** Proveedor por entorno (`JCS_EMBEDDING_URL/MODEL/FORMAT`, Ollama u
OpenAI-compatible). Se guardan como `BLOB` de `f32` y el coseno se calcula sobre un conjunto
acotado (`SEMANTIC_CANDIDATE_LIMIT = 5000`). Por encima de eso hace falta un índice, no un
número más grande. Sin proveedor, la búsqueda sigue funcionando y `jcs memory` lo dice.

**Citas obligatorias.** Todo resultado y todo contexto inyectado lleva `path:start-end`. Sin
cita no hay resultado.

**Incrementalidad.** Hash SHA-256 por archivo: un barrido completo no re-parsea lo que no
cambió. Se indexa al registrar el proyecto, tras promocionar a la base, y en un barrido
periódico. Un *watcher* de sistema de archivos es una optimización posterior, no un
requisito de corrección.

**Context broker.** Antes de enviar el prompt de una tarea, se construye un paquete con los
chunks más relevantes (consulta = objetivo + título de la tarea) y las reglas de política,
bajo un presupuesto de tokens (`JCS_CONTEXT_TOKENS`, 1500 por defecto; 0 lo desactiva). Si el
índice está vacío, no se inyecta nada y el comportamiento es el de antes.

**Medición.** El fixture de evaluación exige **recall@3 = 1.0**: cada consulta debe encontrar
su archivo entre los tres primeros resultados. Sin esa prueba, la memoria no está terminada.

## Consecuencias

**Positivas**
- Funciona sin conexión y sin servicios externos; la semántica se añade cuando se quiere.
- El agente arranca con el código relevante y puede citar de dónde viene.
- El grafo permite responder "¿quién llama a esto?" sin un LSP.
- La indexación es incremental y barata: se puede disparar en cada promoción.

**Negativas / costes**
- SQL no tiene parser: la extracción es heurística y puede perder formas exóticas de DDL.
- El coseno acotado no escala a millones de chunks; el límite está documentado y falla de
  forma visible (se ignoran los no comparados), no silenciosa.
- La extracción de llamadas es por nombre, no por resolución de tipos: puede haber
  homónimos.
- Los embeddings cuestan tiempo y dinero si se activan; por eso están fuera por defecto.

**Riesgos mitigados**
- FTS global mezclando proyectos: la tabla FTS lleva `project_id` y toda consulta filtra.
- Comentarios de cabecera invisibles: detectado por un test de chunking, corregido con un
  chunk de cabecera que termina antes del primer símbolo.
- Contexto que se come el prompt: presupuesto de tokens y se detiene antes de excederlo.

## Verificación

- `crates/jcs-memory`: extracción por lenguaje, chunking (cabecera, anidados, oversize),
  incrementalidad, borrado, búsqueda por símbolo/palabra/SQL, presupuesto de contexto y
  **recall@3 = 1.0** en el fixture.
- `scripts/e2e-memory.sh`: indexar un repo real, buscar con cita, comprobar incrementalidad y
  verificar que el contexto citado llega al prompt de una tarea real (línea
  `injecting code context` en el log del daemon).
