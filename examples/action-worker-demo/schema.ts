/**
 * Example schema demonstrating the full action/worker lifecycle.
 *
 * The Atomo schema parser reads this file and extracts:
 * - Model definitions (interfaces → SQL tables)
 * - Event bindings (which actions fire on create/update/delete)
 * - Action definitions (input types for worker payloads)
 *
 * Run `atomo codegen` to generate `generated/client.ts` from this file.
 */

export interface Post {
  id: string;
  title: string;
  content: string;
  status: string;
  createdAt: Date;
}

export const schema = {
  models: {
    Post: {
      tableName: "posts",
      validation: {
        title: "required|min:1|max:200",
      },
      events: {
        created: [{ action: "processPost" }],
        updated: [
          { action: "onStatusChange", condition: { changedAny: ["status"] } },
        ],
      },
    },
  },
  actions: {
    processPost: {
      input: { pick: { model: "Post", fields: ["id", "title", "content"] } },
    },
    onStatusChange: {
      input: { pick: { model: "Post", fields: ["id", "status"] } },
    },
  },
};

export default schema;
