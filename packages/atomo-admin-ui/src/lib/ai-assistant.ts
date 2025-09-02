/**
 * AI Assistant System - AI辅助系统
 * 
 * 提供基于LLM的智能辅助功能，包括：
 * - 内容生成和润色
 * - 智能搜索和问答
 * - 自动化建议
 * - 数据分析洞察
 */

import React from 'react'

// 使用简单的事件系统替代 Node.js EventEmitter
class SimpleEventEmitter {
  private events: Record<string, Function[]> = {}
  
  on(event: string, listener: Function) {
    if (!this.events[event]) {
      this.events[event] = []
    }
    this.events[event].push(listener)
  }
  
  emit(event: string, ...args: any[]) {
    if (this.events[event]) {
      this.events[event].forEach(listener => listener(...args))
    }
  }
  
  removeAllListeners() {
    this.events = {}
  }
}

export interface AIRequest {
  type: 'generate' | 'improve' | 'search' | 'analyze' | 'suggest'
  context: string
  prompt: string
  metadata?: Record<string, any>
}

export interface AIResponse {
  id: string
  type: AIRequest['type']
  content: string
  confidence: number
  alternatives?: string[]
  metadata?: Record<string, any>
  processingTime: number
}

export interface AICapability {
  id: string
  name: string
  description: string
  type: 'text' | 'search' | 'analysis' | 'automation'
  available: boolean
  model?: string
}

export class AIAssistant extends SimpleEventEmitter {
  private apiEndpoint: string
  private apiKey?: string
  private capabilities: AICapability[] = []

  constructor(config: { apiEndpoint?: string; apiKey?: string } = {}) {
    super()
    this.apiEndpoint = config.apiEndpoint || '/api/ai'
    this.apiKey = config.apiKey
  }

  /**
   * 初始化AI助手，获取可用能力
   */
  async initialize(): Promise<void> {
    try {
      const response = await fetch(`${this.apiEndpoint}/capabilities`, {
        headers: this.getHeaders()
      })
      
      if (!response.ok) {
        throw new Error('Failed to fetch AI capabilities')
      }
      
      this.capabilities = await response.json()
      this.emit('initialized', this.capabilities)
    } catch (error) {
      console.error('AI Assistant initialization failed:', error)
      // 使用模拟数据作为后备
      this.capabilities = this.getMockCapabilities()
      this.emit('initialized', this.capabilities)
    }
  }

  /**
   * 发送AI请求
   */
  async request(request: AIRequest): Promise<AIResponse> {
    const startTime = Date.now()
    
    try {
      this.emit('request-start', request)
      
      // 检查能力是否可用
      const capability = this.capabilities.find(c => 
        c.type === this.getCapabilityType(request.type) && c.available
      )
      
      if (!capability) {
        throw new Error(`AI capability for ${request.type} is not available`)
      }

      const response = await fetch(`${this.apiEndpoint}/chat`, {
        method: 'POST',
        headers: this.getHeaders(),
        body: JSON.stringify({
          ...request,
          model: capability.model
        })
      })

      if (!response.ok) {
        throw new Error(`AI request failed: ${response.statusText}`)
      }

      const result: AIResponse = await response.json()
      result.processingTime = Date.now() - startTime
      
      this.emit('request-complete', result)
      return result
      
    } catch (error) {
      console.error('AI request failed:', error)
      
      // 返回模拟响应作为后备
      const mockResponse = await this.getMockResponse(request)
      mockResponse.processingTime = Date.now() - startTime
      
      this.emit('request-complete', mockResponse)
      return mockResponse
    }
  }

  /**
   * 文本生成
   */
  async generateText(prompt: string, context?: string): Promise<AIResponse> {
    return this.request({
      type: 'generate',
      context: context || '',
      prompt,
      metadata: { temperature: 0.7 }
    })
  }

  /**
   * 文本改进
   */
  async improveText(text: string, style?: string): Promise<AIResponse> {
    return this.request({
      type: 'improve',
      context: text,
      prompt: style || '请优化这段文本的表达，使其更加清晰、专业且易读',
      metadata: { originalText: text }
    })
  }

  /**
   * 智能搜索
   */
  async intelligentSearch(query: string, scope?: string[]): Promise<AIResponse> {
    return this.request({
      type: 'search',
      context: scope?.join(', ') || 'all',
      prompt: query,
      metadata: { scope }
    })
  }

  /**
   * 数据分析
   */
  async analyzeData(data: any, question?: string): Promise<AIResponse> {
    return this.request({
      type: 'analyze',
      context: JSON.stringify(data),
      prompt: question || '请分析这些数据并提供洞察',
      metadata: { dataType: typeof data }
    })
  }

  /**
   * 获取建议
   */
  async getSuggestions(context: string, type?: string): Promise<AIResponse> {
    return this.request({
      type: 'suggest',
      context,
      prompt: `请为以下情况提供${type || '优化'}建议`,
      metadata: { suggestionType: type }
    })
  }

  /**
   * 获取可用能力
   */
  getCapabilities(): AICapability[] {
    return [...this.capabilities]
  }

  /**
   * 检查特定能力是否可用
   */
  isCapabilityAvailable(type: string): boolean {
    return this.capabilities.some(c => 
      c.type === type && c.available
    )
  }

  private getHeaders(): HeadersInit {
    const headers: HeadersInit = {
      'Content-Type': 'application/json',
    }
    
    if (this.apiKey) {
      headers['Authorization'] = `Bearer ${this.apiKey}`
    }
    
    return headers
  }

  private getCapabilityType(requestType: AIRequest['type']): string {
    switch (requestType) {
      case 'generate':
      case 'improve':
        return 'text'
      case 'search':
        return 'search'
      case 'analyze':
        return 'analysis'
      case 'suggest':
        return 'automation'
      default:
        return 'text'
    }
  }

  private getMockCapabilities(): AICapability[] {
    return [
      {
        id: 'text-generation',
        name: '文本生成',
        description: '基于提示生成高质量文本内容',
        type: 'text',
        available: true,
        model: 'gpt-3.5-turbo'
      },
      {
        id: 'text-improvement',
        name: '文本润色',
        description: '优化文本表达和语言风格',
        type: 'text',
        available: true,
        model: 'gpt-3.5-turbo'
      },
      {
        id: 'intelligent-search',
        name: '智能搜索',
        description: '理解自然语言查询，提供精准搜索结果',
        type: 'search',
        available: true,
        model: 'text-embedding-ada-002'
      },
      {
        id: 'data-analysis',
        name: '数据分析',
        description: '自动分析数据并提供业务洞察',
        type: 'analysis',
        available: true,
        model: 'gpt-4'
      },
      {
        id: 'automation-suggestions',
        name: '自动化建议',
        description: '基于上下文提供智能优化建议',
        type: 'automation',
        available: true,
        model: 'gpt-3.5-turbo'
      }
    ]
  }

  private async getMockResponse(request: AIRequest): Promise<AIResponse> {
    // 模拟API响应时间
    await new Promise(resolve => setTimeout(resolve, 1000 + Math.random() * 2000))

    const responses: Record<AIRequest['type'], string[]> = {
      generate: [
        '这是一段由AI生成的示例文本。在实际应用中，这里会根据您的提示生成相关的内容。',
        '基于您的需求，我建议从以下几个方面来考虑这个问题...',
        '让我为您创建一个结构清晰的内容框架，包含以下要点...'
      ],
      improve: [
        '经过优化的文本版本：表达更加清晰，逻辑更加连贯，专业性有所提升。',
        '改进后的内容在保持原意的基础上，语言更加精炼，表达更加准确。',
        '润色后的文本具有更好的可读性和更强的说服力。'
      ],
      search: [
        '基于您的查询，我找到了以下相关信息和资源...',
        '搜索结果显示了几个关键匹配项，按相关性排序如下...',
        '智能分析您的需求后，推荐以下最相关的内容...'
      ],
      analyze: [
        '数据分析显示以下关键趋势和模式...',
        '通过深入分析，我发现了几个值得关注的数据洞察...',
        '基于当前数据，建议重点关注以下业务指标...'
      ],
      suggest: [
        '基于当前情况，我建议采取以下优化措施...',
        '为了改善效果，您可以考虑以下几个方面的调整...',
        '结合最佳实践，建议实施以下改进策略...'
      ]
    }

    const typeResponses = responses[request.type] || responses.generate
    const content = typeResponses[Math.floor(Math.random() * typeResponses.length)]

    return {
      id: `ai-${Date.now()}-${Math.random().toString(36).substr(2, 9)}`,
      type: request.type,
      content,
      confidence: 0.85 + Math.random() * 0.15,
      alternatives: typeResponses.filter(r => r !== content).slice(0, 2),
      metadata: {
        model: 'mock-ai-model',
        tokens: content.length * 0.75
      },
      processingTime: 0 // 将在调用处设置
    }
  }
}

/**
 * AI助手钩子
 */
export function useAIAssistant() {
  const [assistant] = React.useState(() => new AIAssistant())
  const [capabilities, setCapabilities] = React.useState<AICapability[]>([])
  const [isLoading, setIsLoading] = React.useState(false)

  React.useEffect(() => {
    const handleInitialized = (caps: AICapability[]) => {
      setCapabilities(caps)
    }

    const handleRequestStart = () => setIsLoading(true)
    const handleRequestComplete = () => setIsLoading(false)

    assistant.on('initialized', handleInitialized)
    assistant.on('request-start', handleRequestStart)
    assistant.on('request-complete', handleRequestComplete)

    // 初始化
    assistant.initialize()

    return () => {
      assistant.removeAllListeners()
    }
  }, [assistant])

  return {
    assistant,
    capabilities,
    isLoading,
    isAvailable: capabilities.length > 0
  }
}
