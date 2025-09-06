# GraphQL API

Atomo services expose a generated GraphQL API. When running `atomo dev`, visit `/graphql` for the IDE.

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
- Dev server: `http://localhost:3000/graphql` (see service README for ports)

See also: the `schema.ts` in each service (e.g., `services/crm-service/schema.ts`).
