#!/usr/bin/env node

/**
 * Simple code generation script for CRM service
 * This is a temporary solution until the CLI is fixed
 */

const fs = require('fs');
const path = require('path');

// Read the schema file
const schemaPath = path.join(__dirname, 'schema.ts');
const schemaContent = fs.readFileSync(schemaPath, 'utf8');

// Simple regex-based parser for TypeScript interfaces and enums
function parseInterfaces(content) {
  const interfaces = [];
  const enums = [];
  
  // Parse enums first
  const enumRegex = /export enum (\w+) \{([^}]+)\}/g;
  let enumMatch;
  
  while ((enumMatch = enumRegex.exec(content)) !== null) {
    const name = enumMatch[1];
    const body = enumMatch[2];
    
    const values = [];
    const valueRegex = /(\w+)\s*=\s*["']([^"']+)["']/g;
    let valueMatch;
    
    while ((valueMatch = valueRegex.exec(body)) !== null) {
      values.push({
        key: valueMatch[1],
        value: valueMatch[2]
      });
    }
    
    enums.push({ name, values });
  }
  
  // Parse interfaces
  const interfaceRegex = /export interface (\w+) \{([^}]+)\}/g;
  let match;

  while ((match = interfaceRegex.exec(content)) !== null) {
    const name = match[1];
    const body = match[2];

    const fields = [];
    const fieldRegex = /(\w+)(\??):\s*([^;]+);/g;
    let fieldMatch;

    while ((fieldMatch = fieldRegex.exec(body)) !== null) {
      const fieldName = fieldMatch[1];
      const optional = fieldMatch[2] === '?';
      const fieldType = fieldMatch[3].trim();

      fields.push({
        name: fieldName,
        optional,
        fieldType: convertToTypeScript(fieldType)
      });
    }

    interfaces.push({ name, fields });
  }

  return { interfaces, enums };
}

function convertToTypeScript(rustType) {
  // Simple type conversion
  if (rustType === 'string') return 'string';
  if (rustType === 'number') return 'number';
  if (rustType === 'boolean') return 'boolean';
  if (rustType === 'Date') return 'string'; // ISO date string
  if (rustType.includes('[]')) return 'any[]'; // Array types
  if (rustType.startsWith('ContentBlock')) return 'any'; // Platform types
  return rustType;
}

function generateTypes(interfaces, enums) {
  let output = `// Auto-generated TypeScript types for Atomo CRM Service
// Generated from schema.ts - DO NOT EDIT MANUALLY

`;

  // Generate enums first
  for (const enumDef of enums) {
    output += `export enum ${enumDef.name} {\n`;
    for (const value of enumDef.values) {
      output += `  ${value.key} = "${value.value}",\n`;
    }
    output += '}\n\n';
  }

  // Generate interfaces
  for (const interface of interfaces) {
    output += `export interface ${interface.name} {\n`;
    for (const field of interface.fields) {
      const optional = field.optional ? '?' : '';
      output += `  ${field.name}${optional}: ${field.fieldType};\n`;
    }
    output += '}\n\n';

    // Generate input types
    output += `export interface Create${interface.name}Input {\n`;
    for (const field of interface.fields) {
      if (!['id', 'createdAt', 'updatedAt'].includes(field.name)) {
        const optional = field.optional ? '?' : '';
        output += `  ${field.name}${optional}: ${field.fieldType};\n`;
      }
    }
    output += '}\n\n';

    output += `export interface Update${interface.name}Input {\n`;
    for (const field of interface.fields) {
      if (!['id', 'createdAt', 'updatedAt'].includes(field.name)) {
        output += `  ${field.name}?: ${field.fieldType};\n`;
      }
    }
    output += '}\n\n';
  }

  return output;
}

// Parse and generate
const { interfaces, enums } = parseInterfaces(schemaContent);
const typesOutput = generateTypes(interfaces, enums);

// Write to file
const outputPath = path.join(__dirname, 'generated', 'types.ts');
fs.writeFileSync(outputPath, typesOutput);

console.log('✅ Generated types.ts from schema.ts');
