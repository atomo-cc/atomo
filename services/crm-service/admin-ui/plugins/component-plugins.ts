import type { ComponentType, LazyExoticComponent } from 'react';

/**
 * Component Plugin System
 *
 * This system allows dynamic registration of components and routes
 * for the Admin UI.
 */

export interface RouteDefinition {
  pattern: RegExp;
  component: LazyExoticComponent<ComponentType<any>>;
  props: (match?: RegExpMatchArray | undefined) => Record<string, any>;
}

export interface ComponentPlugin {
  name: string;
  components: Record<string, ComponentType<any>>;
  routes: RouteDefinition[];
  init: () => void;
}

export class ComponentPluginManager {
  private plugins: Map<string, ComponentPlugin> = new Map();

  register(plugin: ComponentPlugin) {
    this.plugins.set(plugin.name, plugin);
    plugin.init();
  }

  registerPlugin(plugin: ComponentPlugin) {
    this.register(plugin);
  }

  getPlugin(name: string): ComponentPlugin | undefined {
    return this.plugins.get(name);
  }

  getAllPlugins(): ComponentPlugin[] {
    return Array.from(this.plugins.values());
  }

  getAllRoutes(): RouteDefinition[] {
    const allRoutes: RouteDefinition[] = [];
    for (const plugin of this.plugins.values()) {
      allRoutes.push(...plugin.routes);
    }
    return allRoutes;
  }
}

export const componentPluginManager = new ComponentPluginManager();
