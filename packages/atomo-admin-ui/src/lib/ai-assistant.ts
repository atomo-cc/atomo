/**
 * AI Assistant System
 *
 * Provides LLM-based intelligent assistance, including:
 * - Content generation and polishing
 * - Intelligent search and Q&A
 * - Automation suggestions
 * - Data analysis insights
 */

import React from 'react'

// Use a simple event system instead of the Node.js EventEmitter
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
   * Initialize the AI assistant and fetch the available capabilities.
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
      // Fall back to mock data
      this.capabilities = this.getMockCapabilities()
      this.emit('initialized', this.capabilities)
    }
  }

  /**
   * Send an AI request.
   */
  async request(request: AIRequest): Promise<AIResponse> {
    const startTime = Date.now()
    
    try {
      this.emit('request-start', request)
      
      // Check whether the capability is available
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
      
      // Return a mock response as a fallback
      const mockResponse = await this.getMockResponse(request)
      mockResponse.processingTime = Date.now() - startTime
      
      this.emit('request-complete', mockResponse)
      return mockResponse
    }
  }

  /**
   * Text generation.
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
   * Text improvement.
   */
  async improveText(text: string, style?: string): Promise<AIResponse> {
    return this.request({
      type: 'improve',
      context: text,
      prompt: style || 'Please refine the wording of this text to make it clearer, more professional, and easier to read',
      metadata: { originalText: text }
    })
  }

  /**
   * Intelligent search.
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
   * Data analysis.
   */
  async analyzeData(data: any, question?: string): Promise<AIResponse> {
    return this.request({
      type: 'analyze',
      context: JSON.stringify(data),
      prompt: question || 'Please analyze this data and provide insights',
      metadata: { dataType: typeof data }
    })
  }

  /**
   * Get suggestions.
   */
  async getSuggestions(context: string, type?: string): Promise<AIResponse> {
    return this.request({
      type: 'suggest',
      context,
      prompt: `Please provide ${type || 'optimization'} suggestions for the following situation`,
      metadata: { suggestionType: type }
    })
  }

  /**
   * Get the available capabilities.
   */
  getCapabilities(): AICapability[] {
    return [...this.capabilities]
  }

  /**
   * Check whether a specific capability is available.
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
        name: 'Text Generation',
        description: 'Generate high-quality text content from a prompt',
        type: 'text',
        available: true,
        model: 'gpt-3.5-turbo'
      },
      {
        id: 'text-improvement',
        name: 'Text Polishing',
        description: 'Refine text wording and language style',
        type: 'text',
        available: true,
        model: 'gpt-3.5-turbo'
      },
      {
        id: 'intelligent-search',
        name: 'Intelligent Search',
        description: 'Understand natural-language queries and deliver precise search results',
        type: 'search',
        available: true,
        model: 'text-embedding-ada-002'
      },
      {
        id: 'data-analysis',
        name: 'Data Analysis',
        description: 'Automatically analyze data and provide business insights',
        type: 'analysis',
        available: true,
        model: 'gpt-4'
      },
      {
        id: 'automation-suggestions',
        name: 'Automation Suggestions',
        description: 'Provide intelligent optimization suggestions based on context',
        type: 'automation',
        available: true,
        model: 'gpt-3.5-turbo'
      }
    ]
  }

  private async getMockResponse(request: AIRequest): Promise<AIResponse> {
    // Simulate API response time
    await new Promise(resolve => setTimeout(resolve, 1000 + Math.random() * 2000))

    const responses: Record<AIRequest['type'], string[]> = {
      generate: [
        'This is a sample text generated by AI. In a real application, relevant content would be generated here based on your prompt.',
        'Based on your requirements, I suggest considering this problem from the following angles...',
        'Let me create a clearly structured content outline for you, covering the following key points...'
      ],
      improve: [
        'Optimized version of the text: clearer wording, more coherent logic, and a more professional tone.',
        'The improved content preserves the original meaning while using more concise language and more precise expression.',
        'The polished text reads more smoothly and is more persuasive.'
      ],
      search: [
        'Based on your query, I found the following relevant information and resources...',
        'The search results show several key matches, ranked by relevance as follows...',
        'After intelligently analyzing your needs, here are the most relevant results...'
      ],
      analyze: [
        'The data analysis reveals the following key trends and patterns...',
        'Through in-depth analysis, I identified several noteworthy data insights...',
        'Based on the current data, I recommend focusing on the following business metrics...'
      ],
      suggest: [
        'Based on the current situation, I recommend taking the following optimization measures...',
        'To improve results, you may want to consider adjustments in the following areas...',
        'Drawing on best practices, I recommend implementing the following improvement strategies...'
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
      processingTime: 0 // Set by the caller
    }
  }
}

/**
 * AI assistant hook.
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

    // Initialize
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
