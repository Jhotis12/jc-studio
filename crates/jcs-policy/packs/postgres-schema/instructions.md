# Schema conventions

- Name the constraint: `users_email_key`, `orders_customer_id_fkey`. An anonymous
  constraint cannot be referenced in an error message or dropped deliberately.
- Add columns as nullable, backfill in batches, then add the constraint. Never rewrite a
  large table in a single statement.
- Index columns in the order the query filters, with the most selective first.
- A migration that takes a lock must say for how long and on which table.
- Test the down path only if it exists; otherwise test the forward path twice: on an empty
  database and on a database with data.
