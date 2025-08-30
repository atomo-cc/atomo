/**
 * CRM Service Custom Theme Configuration
 * 
 * This file customizes the Atomo Admin UI theme specifically for the CRM service.
 * It defines colors, typography, and layout preferences.
 */

export const crmTheme = {
  // Brand colors for CRM
  colors: {
    primary: '#1976d2',      // Professional blue
    secondary: '#ff9800',    // Warm orange for accents
    success: '#4caf50',      // Green for won deals
    warning: '#ff9800',      // Orange for urgent items
    error: '#f44336',        // Red for lost deals
    info: '#2196f3',         // Light blue for information
    
    // Deal stage colors
    dealStages: {
      lead: '#e3f2fd',
      qualified: '#f3e5f5',
      proposal: '#fff3e0',
      negotiation: '#fce4ec',
      won: '#e8f5e8',
      lost: '#ffebee'
    }
  },
  
  // Typography
  typography: {
    fontFamily: '"Roboto", "Microsoft YaHei", "PingFang SC", sans-serif',
    fontSize: '14px',
    headings: {
      h1: { fontSize: '2rem', fontWeight: 'bold' },
      h2: { fontSize: '1.5rem', fontWeight: 'semibold' },
      h3: { fontSize: '1.25rem', fontWeight: 'medium' }
    }
  },
  
  // Layout preferences
  layout: {
    sidebarWidth: '240px',
    headerHeight: '64px',
    borderRadius: '8px',
    spacing: {
      xs: '4px',
      sm: '8px',
      md: '16px',
      lg: '24px',
      xl: '32px'
    }
  },
  
  // Component-specific styling
  components: {
    // Data table styling
    dataTable: {
      headerBackground: '#fafafa',
      rowHover: '#f5f5f5',
      borderColor: '#e0e0e0'
    },
    
    // Form styling
    forms: {
      labelColor: '#333',
      inputBorder: '#ddd',
      inputFocus: '#1976d2',
      helpTextColor: '#666'
    },
    
    // Button styling
    buttons: {
      borderRadius: '6px',
      paddingX: '16px',
      paddingY: '8px'
    }
  },
  
  // Dashboard widgets
  dashboard: {
    cardShadow: '0 2px 8px rgba(0,0,0,0.1)',
    chartColors: ['#1976d2', '#ff9800', '#4caf50', '#f44336', '#9c27b0']
  }
};

export default crmTheme;
