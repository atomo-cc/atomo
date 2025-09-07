import { apiClient as atomoApiClient } from '../../../../packages/atomo-admin-ui/src/lib/api'

/**
 * CRM Service API Client
 *
 * CRM服务专用的API客户端，扩展平台Admin UI的API客户端
 * 包含CRM特定的业务逻辑方法
 */

/**
 * CRM Service API Client
 *
 * CRM服务专用的API客户端，包含CRM特定的业务逻辑方法
 */


// CRM特定的API方法
export const crmApiMethods = {
  /**
   * 批量更新Deal位置（以及可选阶段）- CRM特定业务逻辑
   */
  async updateDealPositions(updates: { id: string; position: number; stage?: string }[]): Promise<boolean> {
    const query = `
      mutation UpdateDealPositions($updates: [DealPositionInput!]!) {
        updateDealPositions(updates: $updates)
      }
    `
    const result = await atomoApiClient.graphql(query, { updates })
    return !!result.updateDealPositions
  }
}

// 导出基础API客户端

export const apiClient = atomoApiClient.extend(crmApiMethods)