//! CRM Domain Models - Example Implementation
//! 
//! This module demonstrates how to implement domain models using
//! the Atomo event sourcing framework, as described in the whitepaper.

use atomo_core::{
    traits::Entity,
    events::{DomainEvent, EventMetadata, EventType},
    types::{EntityId, StreamId, Timestamp},
    AtomoError, Result,
};
use crate::aggregate::Aggregate;
use serde::{Deserialize, Serialize};
use chrono::Utc;

/// Customer aggregate - represents a customer in the CRM system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    pub id: EntityId,
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub phone: Option<String>,
    pub status: CustomerStatus,
    pub tags: Vec<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub version: i64,
}

/// Customer status enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CustomerStatus {
    Lead,
    Prospect,
    Customer,
    Inactive,
}

impl Entity for Customer {
    fn id(&self) -> EntityId {
        self.id
    }
    
    fn entity_type(&self) -> &'static str {
        "Customer"
    }
    
    fn created_at(&self) -> Timestamp {
        self.created_at
    }
    
    fn updated_at(&self) -> Timestamp {
        self.updated_at
    }
}

impl Aggregate<CustomerEvent> for Customer {
    fn apply(&mut self, event: &CustomerEvent) -> Result<()> {
        match &event.payload {
            CustomerEventPayload::Created { 
                email, first_name, last_name, company, phone 
            } => {
                self.email = email.clone();
                self.first_name = first_name.clone();
                self.last_name = last_name.clone();
                self.company = company.clone();
                self.phone = phone.clone();
                self.status = CustomerStatus::Lead;
                self.tags = Vec::new();
            }
            
            CustomerEventPayload::Updated { 
                email, first_name, last_name, company, phone 
            } => {
                if let Some(email) = email {
                    self.email = email.clone();
                }
                if let Some(first_name) = first_name {
                    self.first_name = first_name.clone();
                }
                if let Some(last_name) = last_name {
                    self.last_name = last_name.clone();
                }
                if let Some(company) = company {
                    self.company = Some(company.clone());
                }
                if let Some(phone) = phone {
                    self.phone = Some(phone.clone());
                }
            }
            
            CustomerEventPayload::StatusChanged { new_status } => {
                self.status = *new_status;
            }
            
            CustomerEventPayload::TagAdded { tag } => {
                if !self.tags.contains(tag) {
                    self.tags.push(tag.clone());
                }
            }
            
            CustomerEventPayload::TagRemoved { tag } => {
                self.tags.retain(|t| t != tag);
            }
        }
        
        self.updated_at = Utc::now();
        Ok(())
    }
    
    fn version(&self) -> i64 {
        self.version
    }
    
    fn set_version(&mut self, version: i64) {
        self.version = version;
    }
    
    fn is_valid(&self) -> bool {
        !self.email.is_empty() && 
        !self.first_name.is_empty() && 
        !self.last_name.is_empty()
    }
}

/// Customer domain events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerEvent {
    pub aggregate_id: EntityId,
    pub stream_id: StreamId,
    pub aggregate_version: i64,
    pub payload: CustomerEventPayload,
    pub metadata: EventMetadata,
}

/// Customer event payload variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CustomerEventPayload {
    Created {
        email: String,
        first_name: String,
        last_name: String,
        company: Option<String>,
        phone: Option<String>,
    },
    Updated {
        email: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        company: Option<String>,
        phone: Option<String>,
    },
    StatusChanged {
        new_status: CustomerStatus,
    },
    TagAdded {
        tag: String,
    },
    TagRemoved {
        tag: String,
    },
}

impl DomainEvent for CustomerEvent {
    fn stream_id(&self) -> StreamId {
        self.stream_id
    }
    
    fn event_type(&self) -> EventType {
        match self.payload {
            CustomerEventPayload::Created { .. } => EventType::Created,
            CustomerEventPayload::Updated { .. } => EventType::Updated,
            CustomerEventPayload::StatusChanged { .. } => EventType::StateChanged,
            CustomerEventPayload::TagAdded { .. } => EventType::Custom("TagAdded".to_string()),
            CustomerEventPayload::TagRemoved { .. } => EventType::Custom("TagRemoved".to_string()),
        }
    }
    
    fn metadata(&self) -> &EventMetadata {
        &self.metadata
    }
    
    fn payload(&self) -> serde_json::Value {
        serde_json::to_value(&self.payload).unwrap()
    }
    
    fn aggregate_id(&self) -> EntityId {
        self.aggregate_id
    }
    
    fn aggregate_version(&self) -> i64 {
        self.aggregate_version
    }
}

/// Customer commands for business operations
#[derive(Debug, Clone)]
pub enum CustomerCommand {
    Create {
        email: String,
        first_name: String,
        last_name: String,
        company: Option<String>,
        phone: Option<String>,
    },
    Update {
        email: Option<String>,
        first_name: Option<String>,
        last_name: Option<String>,
        company: Option<String>,
        phone: Option<String>,
    },
    ChangeStatus {
        new_status: CustomerStatus,
    },
    AddTag {
        tag: String,
    },
    RemoveTag {
        tag: String,
    },
}

impl Customer {
    /// Create a new customer (factory method)
    pub fn new(
        email: String,
        first_name: String,
        last_name: String,
        company: Option<String>,
        phone: Option<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: EntityId::new(),
            email,
            first_name,
            last_name,
            company,
            phone,
            status: CustomerStatus::Lead,
            tags: Vec::new(),
            created_at: now,
            updated_at: now,
            version: 0,
        }
    }
    
    /// Handle a customer command and return resulting events
    pub fn handle_command(
        &self,
        command: CustomerCommand,
        stream_id: StreamId,
        metadata: EventMetadata,
    ) -> Result<Vec<CustomerEvent>> {
        let mut events = Vec::new();
        
        match command {
            CustomerCommand::Create { 
                email, first_name, last_name, company, phone 
            } => {
                // Validate business rules
                if email.is_empty() || first_name.is_empty() || last_name.is_empty() {
                    return Err(AtomoError::validation("Email, first name, and last name are required"));
                }
                
                events.push(CustomerEvent {
                    aggregate_id: self.id,
                    stream_id,
                    aggregate_version: self.version + 1,
                    payload: CustomerEventPayload::Created {
                        email, first_name, last_name, company, phone
                    },
                    metadata,
                });
            }
            
            CustomerCommand::Update { 
                email, first_name, last_name, company, phone 
            } => {
                // Check if any field is actually changing
                let has_changes = email.is_some() || 
                                 first_name.is_some() || 
                                 last_name.is_some() || 
                                 company.is_some() || 
                                 phone.is_some();
                
                if !has_changes {
                    return Ok(events); // No changes, no events
                }
                
                events.push(CustomerEvent {
                    aggregate_id: self.id,
                    stream_id,
                    aggregate_version: self.version + 1,
                    payload: CustomerEventPayload::Updated {
                        email, first_name, last_name, company, phone
                    },
                    metadata,
                });
            }
            
            CustomerCommand::ChangeStatus { new_status } => {
                if self.status != new_status {
                    events.push(CustomerEvent {
                        aggregate_id: self.id,
                        stream_id,
                        aggregate_version: self.version + 1,
                        payload: CustomerEventPayload::StatusChanged { new_status },
                        metadata,
                    });
                }
            }
            
            CustomerCommand::AddTag { tag } => {
                if !self.tags.contains(&tag) {
                    events.push(CustomerEvent {
                        aggregate_id: self.id,
                        stream_id,
                        aggregate_version: self.version + 1,
                        payload: CustomerEventPayload::TagAdded { tag },
                        metadata,
                    });
                }
            }
            
            CustomerCommand::RemoveTag { tag } => {
                if self.tags.contains(&tag) {
                    events.push(CustomerEvent {
                        aggregate_id: self.id,
                        stream_id,
                        aggregate_version: self.version + 1,
                        payload: CustomerEventPayload::TagRemoved { tag },
                        metadata,
                    });
                }
            }
        }
        
        Ok(events)
    }
    
    /// Business rule: Can customer be converted to prospect?
    pub fn can_convert_to_prospect(&self) -> bool {
        self.status == CustomerStatus::Lead && 
        self.company.is_some() && 
        self.phone.is_some()
    }
    
    /// Business rule: Can customer be activated?
    pub fn can_activate(&self) -> bool {
        matches!(self.status, CustomerStatus::Lead | CustomerStatus::Prospect | CustomerStatus::Inactive)
    }
}
