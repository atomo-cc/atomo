/**
 * CRM Data Schema Definition
 * 
 * This file defines the complete data model for our CRM system.
 * Atomo will automatically generate:
 * - Database tables and relationships
 * - GraphQL types and resolvers
 * - Admin UI forms and views
 * - TypeScript types for the frontend
 */

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
  notes: Block[];  // Rich content using Atomo's composable content system
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
  notes: Block[];  // Rich content blocks
  createdAt: Date;
  updatedAt: Date;
}

export interface Deal {
  id: string;
  title: string;
  value: number;
  stage: DealStage;
  contactId: string;
  companyId?: string;
  description: Block[];  // Rich content blocks
  expectedCloseDate?: Date;
  actualCloseDate?: Date;
  createdAt: Date;
  updatedAt: Date;
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
// Atomo Composable Content Blocks
// =============================================================================

/**
 * Atomo's "流动的画布" - Composable content blocks that can be used
 * in any rich content field (notes, descriptions, etc.)
 */
export type Block = 
  | ParagraphBlock
  | CallLogBlock  
  | MeetingNoteBlock
  | TaskBlock;

export interface ParagraphBlock {
  type: "paragraph";
  content: string;
}

export interface CallLogBlock {
  type: "call_log";
  duration: number; // in minutes
  outcome: string;
  notes: string;
  recordedAt: Date;
}

export interface MeetingNoteBlock {
  type: "meeting_note";
  title: string;
  attendees: string[];
  agenda: string;
  notes: string;
  actionItems: string[];
  meetingDate: Date;
}

export interface TaskBlock {
  type: "task";
  title: string;
  description?: string;
  assignedTo?: string;
  dueDate?: Date;
  completed: boolean;
}

// =============================================================================
// Schema Metadata for Atomo Platform
// =============================================================================

/**
 * This metadata tells Atomo how to handle the schema:
 * - Relationships between models
 * - Validation rules
 * - UI generation hints
 * - Search and indexing configuration
 */
export const schema = {
  models: {
    Contact: {
      tableName: 'contacts',
      primaryKey: 'id',
      searchable: ['firstName', 'lastName', 'email'],
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
        editForm: ['title', 'value', 'stage', 'contactId', 'companyId', 'description', 'expectedCloseDate']
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
