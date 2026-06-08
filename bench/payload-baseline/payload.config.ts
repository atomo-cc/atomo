import { buildConfig } from "payload";
import { postgresAdapter } from "@payloadcms/db-postgres";

export default buildConfig({
  secret: "bench-secret-not-for-production-use-only",
  db: postgresAdapter({
    pool: { connectionString: process.env.DATABASE_URL! },
    push: false,
  }),
  collections: [
    {
      slug: "bench-payload-notes",
      timestamps: true,
      // Disable auth-related hooks and access control for clean bench measurements
      access: { read: () => true, create: () => true, update: () => true, delete: () => true },
      fields: [{ name: "title", type: "text" }],
    },
    {
      slug: "bench-payload-tags",
      timestamps: true,
      access: { read: () => true, create: () => true, update: () => true, delete: () => true },
      fields: [
        { name: "label", type: "text" },
        {
          name: "note",
          type: "relationship",
          relationTo: "bench-payload-notes",
        },
      ],
    },
  ],
  telemetry: false,
});
