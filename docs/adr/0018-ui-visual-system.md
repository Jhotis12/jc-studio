# 0018 — Sistema visual de la app: densidad legible, no decoración

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

La app funcionaba pero se leía como el tema oscuro por defecto de egui: cuerpo 12.5,
`small` 9, pestañas de texto sin jerarquía, tarjetas planas y un compositor de objetivos que
ocupaba casi todo el alto útil. En una ventana de 941×568 el tablero no dejaba ver los
objetivos, el explorador de Código desbordaba sus controles sobre el editor y las etiquetas
de estado/timestamps eran difíciles de leer.

## Dirección

Consola de operaciones: densa, tranquila y escaneable. No es una landing page. El detalle
memorable es una **superficie única con acento frío**: el trabajo vivo usa azul, el éxito
verde, el aviso ámbar y el fallo rojo; todo lo demás es fondo, borde y texto.

## Decisión

- `ui/theme.rs` concentra tokens de color, radios, marcos y la escala tipográfica. Ningún
  panel inventa un color suelto para un estado.
- Escala: `Heading` 21, `Body` 14, `Monospace` 13, `Button` 13.5, `Small` 11.5. Los metadatos
  no bajan de 11 y usan un gris con contraste ≥ 5:1 sobre las superficies.
- Superficies: `BG` para la ventana, `SURFACE` para laterales/tarjetas, `SURFACE_RAISED`
  para lo seleccionado o los controles, `SURFACE_SUNKEN` para editor/terminal. Bordes de 1 px;
  el seleccionado usa borde de acento, no un relleno que baje el contraste.
- Shell: barra superior de 56 px con marca, estado y gasto; navegación en una barra propia
  con pestañas tipo pill y scroll horizontal si no caben; laterales con superficie y padding.
- Ancho: tres paneles solo a partir de **1160 px**. Por debajo, el detalle es una pestaña;
  así el tablero y el código conservan ancho útil.
- Tablero: el compositor empieza **colapsado cuando hay objetivos** y se abre con
  «+ Nuevo objetivo». La lista manda. Las tarjetas muestran estado, coste, progreso por
  tarea, conteos y la actividad actual; no una etiqueta por cada tarea.
- Consola inferior: 150 px por defecto (mínimo 90, máximo 380), pestañas propias y cierre
  explícito.
- Código: explorador en tarjeta de ancho fijo compacto; el selector de worktree sustituye al
  checkbox; crear archivo vive detrás de un `+`; el editor vacío explica qué está viendo.
- Detalle: cabecera en tarjeta con estado, prompt, coste/tokens, barra por tarea y conteos;
  las tareas son tarjetas con «Ahora»/«Última actividad», no una línea de depuración.

## Consecuencias

- **A favor:** la misma información cabe en menos alto; el texto se lee; los estados se
  distinguen por color y por etiqueta; el foco y los targets son visibles; el tablero deja de
  parecer vacío cuando hay trabajo.
- **En contra:** más código de presentación que mantener; los paneles con pocos datos
  (Política, Sistema) siguen teniendo espacio vacío, porque no se inventan métricas. La
  tipografía del sistema la aporta egui; no se bundlea una fuente nueva.
- Los umbrales son de escritorio: por debajo de 980×640 no se garantiza una composición
  usable, y la app declara ese mínimo.

## Verificación

- Capturas revisadas a 941×568 (layout de pestañas) y 1380×880 (tres paneles): sin
  solapamientos en navegación, Código, Orquestador, Detalle ni Sistema.
- Contraste calculado: texto 15.9:1, muted 7.5:1, faint 5.5:1, acento 8.9:1 sobre `BG`.
- `cargo nextest run --workspace`: 345 tests; `cargo clippy --workspace --all-targets
  -- -D warnings` y `cargo deny check` limpios.
