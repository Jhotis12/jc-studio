# 0014 — Coste calculado desde el catálogo cuando el backend reporta cero

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

La app mostraba $0.0000 con más de un millón de tokens consumidos. El modelo era
`opencode-go/deepseek-v4.1-flash`, cuyo precio de catálogo es $0.15/M de entrada y $0.60/M
de salida. OpenCode enviaba `session.usage.updated` con los tokens pero `cost: 0`,
probablemente porque la cuenta usa créditos o un proveedor que no reporta precio.

Confiar en el `cost` del backend deja el gasto invisible. Ignorarlo y calcular siempre
podría contradecir un descuento real. Hay que distinguir los dos casos.

## Alternativas consideradas

1. **Seguir mostrando el coste del backend.** Simple y erróneo en este caso; el operador no
   puede ver el gasto.
2. **Calcular siempre desde el catálogo.** Predecible, pero ignora promociones o precios
   reales que el backend sí conoce.
3. **Confiar en un coste positivo y calcular solo cuando sea 0.** Conserva el dato del
   proveedor cuando existe y cubre el caso de suscripción/créditos.

## Decisión

- `runs` guarda `model`, `cost_input_per_mtok` y `cost_output_per_mtok` (migración 0008).
  El ejecutor los rellena al resolver el modelo, antes de crear la sesión del agente.
- `set_run_usage` usa el coste reportado si es **positivo**; si es 0, calcula
  `(tokens_in * precio_in + tokens_out * precio_out) / 1_000_000`. Un modelo gratuito tiene
  tasas 0 y permanece en $0.
- Al arrancar, `costs::spawn_backfill` recorre los runs con tokens y coste 0, busca el modelo
  del goal en el catálogo y actualiza el coste. Los runs antiguos de este proyecto pasaron de
  $0.0000 a **$0.2375**.
- La UI aclara que el total cubre los runs de tareas; las sesiones del planificador y del
  revisor todavía no tienen un run asociado y no se suman.

## Consecuencias

- **A favor:** el gasto es visible sin depender de que el proveedor lo reporte; los modelos
  gratuitos siguen en cero; el modelo queda registrado en cada run para auditoría.
- **En contra:** si el backend aplica un descuento y reporta 0, el cálculo usa el precio de
  lista (conservador). Si el modelo no está en el catálogo o no se especificó, el coste queda
  en 0 y se registra un aviso. Los precios no se recalculan si el catálogo cambia después.
- El planificador y el revisor quedan fuera hasta que exista un registro de uso auxiliar
  (anotado en `PENDING.md`).

## Verificación

- `cargo test -p jcs-store pricing_tests`: coste 0 reportado con tasas 1/2 y 1 M/0.5 M tokens
  da $2.00; un coste positivo reportado se conserva.
- Tras el despliegue, `sum(cost_usd)` de los cinco runs existentes es **0.2375** y
  `/status.totals.cost_usd` coincide.
- La app muestra `$0.2375` en la barra superior y en la tarjeta del objetivo.
