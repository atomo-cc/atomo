# GraphQL API

Atomo services expose a generated GraphQL API. When running `atomo dev`, visit `/graphql` for the IDE. The server merges service queries with platform queries (users, sessions, audit).

## Example Operations

```graphql
# Query contacts with relations
query Contacts {
  contacts(where: { email: { contains: "@example.com" } }) {
    id
    firstName
    lastName
    company { id name }
  }
}
```

```graphql
# Create a contact
mutation CreateContact($input: CreateContactInput!) {
  createContact(input: $input) { id email }
}
```

## Local Endpoints
- Dev server default: `http://localhost:3000/graphql`
- Override with `atomo dev --port <n>` or `atomo-server --port <n>`

See also: the `schema.ts` in each service (e.g., `services/crm-service/schema.ts`).
