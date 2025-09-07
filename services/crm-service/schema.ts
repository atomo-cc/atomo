/**
 * CRM Data Schema Definition
 * 
 * This file defines the complete data model for our CRM system.
 * Atomo will automatically generate:
 * - Database tables and relationships
 * - GraphQL types and resolvers
 * - Admin UI forms and views
 * - TypeScript types for the frontend
 * 
 * Note: Platform-level models (User, Session, UserRole) are now defined
 * in atomo_core and will be automatically available in all services.
 */

// Import platform-level types from atomo_core
// These will be generated and made available by the Atomo platform
type ContentBlock = any; // Placeholder for platform-generated ContentBlock type

// =============================================================================
// Specialized Content Block Types for CRM
// =============================================================================

export interface ParagraphBlock {
  id: string;
  text: string;
  formatting?: any;
  order: number;
}

export interface CallLogBlock {
  id: string;
  duration?: number;
  call_time: Date;
  notes: string;
  outcome?: string;
  order: number;
}

export interface MeetingNoteBlock {
  id: string;
  title: string;
  meeting_time: Date;
  attendees: string[];
  notes: string;
  action_items: string[];
  order: number;
}

export interface TaskBlock {
  id: string;
  title: string;
  description?: string;
  due_date?: Date;
  priority?: number;
  is_completed: boolean;
  assigned_to?: string;
  order: number;
}

// =============================================================================
// Core Customer Models
// =============================================================================

export interface Contact {
  id: string;
  firstName: string;
  lastName: string;
  email: string;
  phone?: string;
  companyId?: string;
  tags: string[];
  notes: ContentBlock[];  // Using platform-level ContentBlock from atomo_core
  createdAt: Date;
  updatedAt: Date;
}

export interface Company {
  id: string;
  name: string;
  website?: string;
  address?: string;
  industry?: string;
  size?: CompanySize;
  notes: ContentBlock[];  // Using platform-level ContentBlock from atomo_core
  createdAt: Date;
  updatedAt: Date;
}

export interface Deal {
  id: string;
  title: string;
  value: number;
  stage: DealStage;
  // Position within a stage for Kanban ordering
  position: number;
  contactId: string;
  companyId?: string;
  description: ContentBlock[];  // Using platform-level ContentBlock from atomo_core
  expectedCloseDate?: Date;
  actualCloseDate?: Date;
  createdAt: Date;
  updatedAt: Date;
}

// Activity timeline for contacts (calls, meetings, notes, emails, tasks)
export interface Activity {
  id: string;
  contactId: string;
  activityType: ActivityType;
  title?: string;
  content?: string;
  metadata?: any;
  createdAt: Date;
  updatedAt: Date;
}

export enum ActivityType {
  NOTE = "note",
  CALL = "call",
  MEETING = "meeting",
  EMAIL = "email",
  TASK = "task",
}

// =============================================================================
// Supporting Enums and Types
// =============================================================================

export enum CompanySize {
  STARTUP = "startup",
  SMALL = "small", 
  MEDIUM = "medium",
  LARGE = "large",
  ENTERPRISE = "enterprise"
}

export enum DealStage {
  LEAD = "lead",
  QUALIFIED = "qualified", 
  PROPOSAL = "proposal",
  NEGOTIATION = "negotiation",
  WON = "won",
  LOST = "lost"
}

// =============================================================================
// Schema Metadata for Atomo Platform
// =============================================================================

/**
 * This metadata tells Atomo how to handle the CRM business models:
 * - Relationships between models
 * - Validation rules
 * - UI generation hints
 * - Search and indexing configuration
 * 
 * Note: Platform-level models (User, Session) are configured in atomo_core
 */
export const schema = {
  models: {
    Contact: {
      tableName: 'contacts',
      primaryKey: 'id',
      searchable: ['firstName', 'lastName', 'email'],
      access: {
        create: 'sales|manager|admin',
        read: 'authenticated',
        update: 'sales|manager|admin',
        delete: 'manager|admin'
      },
      relationships: {
        company: {
          type: 'belongsTo',
          model: 'Company',
          foreignKey: 'companyId'
        },
        deals: {
          type: 'hasMany',
          model: 'Deal',
          foreignKey: 'contactId'
        }
      },
      validation: {
        email: 'email',
        firstName: 'required|min:1|max:100',
        lastName: 'max:100'
      },
      ui: {
        displayField: ['firstName', 'lastName'],
        listView: ['firstName', 'lastName', 'email', 'company', 'createdAt'],
        editForm: ['firstName', 'lastName', 'email', 'phone', 'companyId', 'tags', 'notes']
      }
    },
    
    Company: {
      tableName: 'companies',
      primaryKey: 'id',
      searchable: ['name', 'website', 'industry'],
      access: {
        create: 'sales|manager|admin',
        read: 'authenticated',
        update: 'sales|manager|admin',
        delete: 'manager|admin'
      },
      relationships: {
        contacts: {
          type: 'hasMany',
          model: 'Contact',
          foreignKey: 'companyId'
        },
        deals: {
          type: 'hasMany',
          model: 'Deal',
          foreignKey: 'companyId'
        }
      },
      validation: {
        name: 'required|min:1|max:255',
        website: 'url',
        email: 'email'
      },
      ui: {
        displayField: 'name',
        listView: ['name', 'website', 'industry', 'size', 'createdAt'],
        editForm: ['name', 'website', 'address', 'industry', 'size', 'notes']
      }
    },
    
    Deal: {
      tableName: 'deals',
      primaryKey: 'id',
      searchable: ['title'],
      access: {
        create: 'sales|manager|admin',
        read: 'authenticated',
        update: 'sales|manager|admin',
        delete: 'manager|admin'
      },
      relationships: {
        contact: {
          type: 'belongsTo',
          model: 'Contact',
          foreignKey: 'contactId'
        },
        company: {
          type: 'belongsTo',
          model: 'Company',
          foreignKey: 'companyId'
        }
      },
      validation: {
        title: 'required|min:1|max:255',
        value: 'numeric|min:0',
        contactId: 'required|exists:contacts,id'
      },
      ui: {
        displayField: 'title',
        listView: ['title', 'value', 'stage', 'contact', 'company', 'expectedCloseDate'],
        editForm: ['title', 'value', 'stage', 'position', 'contactId', 'companyId', 'description', 'expectedCloseDate']
      }
    },

    Activity: {
      tableName: 'activity',
      primaryKey: 'id',
      searchable: ['title', 'content', 'activityType'],
      access: {
        create: 'sales|manager|admin',
        read: 'authenticated',
        update: 'sales|manager|admin',
        delete: 'manager|admin'
      },
      relationships: {
        contact: {
          type: 'belongsTo',
          model: 'Contact',
          foreignKey: 'contactId'
        }
      },
      validation: {
        contactId: 'required|exists:contacts,id',
        activityType: 'required|in:note,call,meeting,email,task'
      },
      ui: {
        displayField: 'title',
        listView: ['activityType', 'title', 'contact', 'createdAt'],
        editForm: ['activityType', 'title', 'content', 'contactId']
      }
    }
  },
  
  // Global configuration
  config: {
    // Enable audit logging for all models
    auditLog: true,
    
    // Enable soft deletes
    softDeletes: true,
    
    // Default pagination size
    defaultPageSize: 20,
    
    // Enable real-time subscriptions
    subscriptions: true
  }
};

export default schema;
