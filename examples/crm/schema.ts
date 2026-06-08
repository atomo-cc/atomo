/**
 * CRM Schema — multi-model example exercising events, RBAC, validation,
 * relationships, and cross-model worker cooperation.
 *
 * Models: Contact, Company, Deal, Activity
 * Workers: contact-worker (onNewContact, onStageChange)
 *          deal-worker    (onDealStatusChange)
 */

// ── Enums ───────────────────────────────────────────────────────────────────

type ContactStage = "lead" | "qualified" | "customer";
type DealStatus = "open" | "won" | "lost";
type ActivityType = "call" | "email" | "meeting";

// ── Model interfaces ────────────────────────────────────────────────────────

export interface Company {
  id: string;
  createdAt: Date;
  industry?: string;
  name: string;
  updatedAt: Date;
  website?: string;
}

export interface Contact {
  id: string;
  companyId?: string;
  createdAt: Date;
  email: string;
  name: string;
  phone?: string;
  stage: ContactStage;
  updatedAt: Date;
}

export interface Deal {
  id: string;
  contactId: string;
  createdAt: Date;
  status: DealStatus;
  title: string;
  updatedAt: Date;
  value: number;
}

export interface Activity {
  id: string;
  contactId: string;
  createdAt: Date;
  dealId?: string;
  notes?: string;
  type: ActivityType;
  updatedAt: Date;
}

// ── Schema metadata ─────────────────────────────────────────────────────────

export const schema = {
  models: {
    Company: {
      tableName: "companies",
      validation: {
        name: "required",
        website: "url",
      },
      access: {
        create: "sales|admin",
        read: "authenticated",
      },
    },

    Contact: {
      tableName: "contacts",
      validation: {
        email: "email",
        name: "required|min:1|max:100",
      },
      access: {
        create: "sales|admin",
        read: "authenticated",
        delete: "admin",
      },
      relationships: {
        company: {
          type: "belongsTo",
          model: "Company",
          foreignKey: "companyId",
        },
      },
      events: {
        created: [{ action: "onNewContact" }],
        updated: [
          {
            action: "onStageChange",
            condition: { changedAny: ["stage"] },
          },
        ],
      },
    },

    Deal: {
      tableName: "deals",
      validation: {
        title: "required",
        value: "min:0",
      },
      access: {
        create: "sales|admin",
        read: "authenticated",
        update: "sales|admin",
        delete: "admin",
      },
      relationships: {
        contact: {
          type: "belongsTo",
          model: "Contact",
          foreignKey: "contactId",
        },
      },
      events: {
        updated: [
          {
            action: "onDealStatusChange",
            condition: { changedAny: ["status"] },
          },
        ],
      },
    },

    Activity: {
      tableName: "activities",
      access: {
        create: "sales|admin",
        read: "authenticated",
      },
      relationships: {
        contact: {
          type: "belongsTo",
          model: "Contact",
          foreignKey: "contactId",
        },
        deal: {
          type: "belongsTo",
          model: "Deal",
          foreignKey: "dealId",
        },
      },
    },
  },

  actions: {
    onNewContact: {
      input: { pick: { model: "Contact", fields: ["id", "name", "email"] } },
    },
    onStageChange: {
      input: { pick: { model: "Contact", fields: ["id", "stage"] } },
    },
    onDealStatusChange: {
      input: { pick: { model: "Deal", fields: ["id", "status", "value"] } },
    },
  },
};
