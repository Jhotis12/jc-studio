# 0012 — Secretos en archivo 0600, fuera de SQLite y de la API

- **Estado:** Accepted
- **Fecha:** 2026-11-21

## Contexto

El operador quiere configurar el bot de Telegram desde Ajustes, no con variables de entorno.
El token es una credencial: no puede acabar en la tabla `settings`, en los logs, en un evento
de auditoría ni en una respuesta HTTP que luego se muestre en una captura.

No hay garantía de que el escritorio tenga un servicio de llavero (Secret Service) activo, y
el daemon debe arrancar igualmente.

## Alternativas consideradas

1. **Tabla `settings`.** Cómoda, pero guarda el token en claro en la misma base que se copia
   en los backups y se inspecciona con `sqlite3`. Rechazada.
2. **Solo variables de entorno.** Segura para el repositorio, pero obliga a editar la unidad
   de systemd y reiniciar; contradice «configurar desde la interfaz». Se conserva como valor
   inicial.
3. **Llavero del sistema (Secret Service).** Es el destino preferente, pero añade una
   dependencia de D-Bus y un modo de fallo en escritorios mínimos. Queda como siguiente paso.
4. **Archivo privado con escritura atómica.** Mismo usuario, permisos 0600, sin dependencias.
   El daemon ya corre como el usuario y su frontera de seguridad es el socket 0600.

## Decisión

- `~/.local/share/jcs/secrets.json`, escrito de forma atómica (temporal + `rename`),
  permisos **0600** antes del rename.
- La API es **write-only**: `SettingsPatch.telegram_token` escribe o borra; `SettingsView`
  solo devuelve `telegram_configured` y `telegram_token_hint` (últimos cuatro caracteres).
- Un archivo con `"telegram_token": null` es un «apagado» explícito y **no** cae de vuelta a
  la variable de entorno, para que borrar el campo en la UI realmente desactive el bot.
- Los chats permitidos no son secretos: viven en `settings` y se validan (máximo 20, sin
  cero).
- El bot se arranca al iniciar `jcsd`; un cambio de token o chats marca «requiere reiniciar»
  y la app ofrece el botón. Reiniciar interrumpe runs en curso, que quedan `interrupted` y
  sus tareas `blocked`.
- El token nunca se registra: ni en `Debug`, ni en errores, ni en eventos. `settings_changed`
  solo enumera las claves cambiadas.

## Consecuencias

- **A favor:** configuración desde la UI sin meter el token en la base; backups de la base no
  filtran la credencial; la API puede mostrarse sin riesgo.
- **En contra:** el archivo no está cifrado en reposo; cualquier proceso del mismo usuario
  puede leerlo (igual que el socket). Hay que incluirlo en el backup si se quiere conservar.
  Migrar al llavero es un cambio de implementación, no de contrato.
- Si el archivo se corrompe, el daemon arranca con secretos desactivados y lo registra; no
  impide servir el resto de la aplicación.

## Verificación

- `cargo test -p jcs-daemon secrets`: escribe con modo 0600, recarga, borra sin volver a la
  variable de entorno y el hint no contiene el token completo.
- `the_telegram_token_is_write_only` (HTTP): `PUT` con token, `GET` sin el token, con
  `telegram_configured=true`, hint y chats; el cuerpo no contiene el secreto.
- `stored_values_keep_new_fields_from_the_environment`: una fila antigua de `settings` no
  borra los chats que vinieron del entorno.
