# Platform GraphQL

Service GraphQL is merged with platform-level queries and mutations.

## Queries
```graphql
# List users
query($limit: Int, $offset: Int) {
  users(limit: $limit, offset: $offset) {
    id email role is_active created_at
  }
}

# User sessions
query($userId: String) {
  user_sessions(user_id: $userId, limit: 20) {
    id user_id expires_at ip_address user_agent
  }
}

# Audit logs
query {
  audit_logs(entity_type: "Contact", limit: 10) {
    id entity_type entity_id operation created_at
  }
}
```

## Mutations
```graphql
mutation {
  create_user(email: "a@b.com", password: "secret", role: Viewer) { id email role }
}

mutation {
  update_user(id: "...", role: Manager, is_active: true) { id email role is_active }
}
```
