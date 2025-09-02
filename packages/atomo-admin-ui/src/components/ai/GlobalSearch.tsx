/**
 * Global Search - 全局智能搜索组件
 * 
 * 基于LLM的全局知识库搜索，提供：
 * - 自然语言查询
 * - 跨模型智能搜索
 * - 上下文感知结果
 * - 问答式交互
 */

import React, { useState, useRef, useEffect } from 'react'
import { 
  Search, 
  Brain, 
  Filter, 
  Clock, 
  Star,
  MessageCircle,
  FileText,
  Users,
  Building,
  Zap,
  ArrowRight,
  Sparkles,
  X
} from 'lucide-react'

import { Button } from '../ui/Button'
import { Card, CardContent } from '../ui/Card'
import { Badge } from '../ui/Badge'
import { Input } from '../ui/Input'
import { useAIAssistant } from '../../lib/ai-assistant'
import { cn, formatDate } from '../../lib/utils'

interface SearchResult {
  id: string
  type: 'entity' | 'document' | 'workflow' | 'user' | 'insight'
  title: string
  content: string
  score: number
  metadata: {
    entityType?: string
    lastModified?: Date
    author?: string
    tags?: string[]
  }
  context?: string
}

interface SearchSuggestion {
  id: string
  text: string
  type: 'query' | 'filter' | 'action'
  icon: React.ReactNode
}

interface GlobalSearchProps {
  isOpen: boolean
  onClose: () => void
  initialQuery?: string
}

export function GlobalSearch({ isOpen, onClose, initialQuery = '' }: GlobalSearchProps) {
  const [query, setQuery] = useState(initialQuery)
  const [results, setResults] = useState<SearchResult[]>([])
  const [suggestions, setSuggestions] = useState<SearchSuggestion[]>([])
  const [isSearching, setIsSearching] = useState(false)
  const [selectedFilter, setSelectedFilter] = useState<string>('all')
  const [recentSearches, setRecentSearches] = useState<string[]>([])
  const { assistant, isAvailable } = useAIAssistant()
  const inputRef = useRef<HTMLInputElement>(null)

  // 搜索过滤器
  const searchFilters = [
    { id: 'all', label: '全部', icon: <Search className="h-4 w-4" /> },
    { id: 'contacts', label: '联系人', icon: <Users className="h-4 w-4" /> },
    { id: 'companies', label: '公司', icon: <Building className="h-4 w-4" /> },
    { id: 'documents', label: '文档', icon: <FileText className="h-4 w-4" /> },
    { id: 'workflows', label: '工作流', icon: <Zap className="h-4 w-4" /> }
  ]

  // 聚焦输入框
  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus()
    }
  }, [isOpen])

  // 初始化建议
  useEffect(() => {
    if (isOpen && !query) {
      setSuggestions(getInitialSuggestions())
    }
  }, [isOpen, query])

  // 加载最近搜索
  useEffect(() => {
    const recent = localStorage.getItem('recent-searches')
    if (recent) {
      setRecentSearches(JSON.parse(recent).slice(0, 5))
    }
  }, [])

  const handleSearch = async (searchQuery?: string) => {
    const finalQuery = searchQuery || query
    if (!finalQuery.trim()) return

    setIsSearching(true)
    
    try {
      // 保存到最近搜索
      const updatedRecent = [finalQuery, ...recentSearches.filter(q => q !== finalQuery)].slice(0, 5)
      setRecentSearches(updatedRecent)
      localStorage.setItem('recent-searches', JSON.stringify(updatedRecent))

      if (isAvailable) {
        // 使用AI智能搜索
        const aiResponse = await assistant.intelligentSearch(
          finalQuery,
          selectedFilter === 'all' ? undefined : [selectedFilter]
        )
        
        // 解析AI响应并转换为搜索结果
        const results = await parseAISearchResponse(aiResponse.content, finalQuery)
        setResults(results)
      } else {
        // 后备搜索
        const results = await performFallbackSearch(finalQuery, selectedFilter)
        setResults(results)
      }

      setSuggestions([])
    } catch (error) {
      console.error('搜索失败:', error)
      // 显示错误状态或使用后备搜索
      const fallbackResults = await performFallbackSearch(finalQuery, selectedFilter)
      setResults(fallbackResults)
    } finally {
      setIsSearching(false)
    }
  }

  const handleQueryChange = (value: string) => {
    setQuery(value)
    
    // 动态生成建议
    if (value.length > 0) {
      const newSuggestions = generateQuerySuggestions(value)
      setSuggestions(newSuggestions)
    } else {
      setSuggestions(getInitialSuggestions())
      setResults([])
    }
  }

  const handleSuggestionClick = (suggestion: SearchSuggestion) => {
    if (suggestion.type === 'query') {
      setQuery(suggestion.text)
      handleSearch(suggestion.text)
    } else if (suggestion.type === 'filter') {
      setSelectedFilter(suggestion.id)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      handleSearch()
    } else if (e.key === 'Escape') {
      onClose()
    }
  }

  const getResultIcon = (type: SearchResult['type']) => {
    switch (type) {
      case 'entity': return <Users className="h-4 w-4 text-blue-600" />
      case 'document': return <FileText className="h-4 w-4 text-green-600" />
      case 'workflow': return <Zap className="h-4 w-4 text-purple-600" />
      case 'user': return <Users className="h-4 w-4 text-orange-600" />
      case 'insight': return <Brain className="h-4 w-4 text-pink-600" />
      default: return <Search className="h-4 w-4 text-gray-600" />
    }
  }

  const getResultTypeLabel = (type: SearchResult['type']) => {
    const labels = {
      entity: '实体',
      document: '文档',
      workflow: '工作流',
      user: '用户',
      insight: '洞察'
    }
    return labels[type] || '未知'
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black bg-opacity-50 pt-20">
      <Card className="w-full max-w-2xl max-h-[80vh] overflow-hidden">
        <CardContent className="p-0">
          {/* 搜索头部 */}
          <div className="p-4 border-b border-gray-200">
            <div className="flex items-center gap-3">
              <div className="flex-1 relative">
                <Brain className="absolute left-3 top-1/2 transform -translate-y-1/2 h-5 w-5 text-purple-600" />
                <Input
                  ref={inputRef}
                  value={query}
                  onChange={(e) => handleQueryChange(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder="智能搜索：询问任何关于您业务的问题..."
                  className="pl-11 pr-4 py-3 text-base"
                />
                {isSearching && (
                  <div className="absolute right-3 top-1/2 transform -translate-y-1/2">
                    <div className="animate-spin rounded-full h-4 w-4 border-2 border-purple-600 border-t-transparent" />
                  </div>
                )}
              </div>
              <Button variant="ghost" onClick={onClose}>
                <X className="h-5 w-5" />
              </Button>
            </div>

            {/* 过滤器 */}
            <div className="flex items-center gap-2 mt-3 overflow-x-auto">
              {searchFilters.map((filter) => (
                <button
                  key={filter.id}
                  onClick={() => setSelectedFilter(filter.id)}
                  className={cn(
                    "flex items-center gap-2 px-3 py-1 rounded-full text-sm transition-colors whitespace-nowrap",
                    selectedFilter === filter.id
                      ? "bg-purple-100 text-purple-700 border border-purple-300"
                      : "bg-gray-100 text-gray-700 hover:bg-gray-200"
                  )}
                >
                  {filter.icon}
                  {filter.label}
                </button>
              ))}
            </div>
          </div>

          {/* 搜索内容 */}
          <div className="max-h-96 overflow-y-auto">
            {/* 搜索结果 */}
            {results.length > 0 && (
              <div className="p-4 space-y-3">
                <div className="flex items-center gap-2 text-sm text-gray-600">
                  <Sparkles className="h-4 w-4" />
                  找到 {results.length} 个相关结果
                </div>
                
                {results.map((result, index) => (
                  <div
                    key={result.id}
                    className="p-3 border border-gray-200 rounded-md hover:bg-gray-50 cursor-pointer transition-colors"
                  >
                    <div className="flex items-start justify-between">
                      <div className="flex-1">
                        <div className="flex items-center gap-2 mb-2">
                          {getResultIcon(result.type)}
                          <span className="font-medium text-gray-900">{result.title}</span>
                          <Badge variant="secondary" className="text-xs">
                            {getResultTypeLabel(result.type)}
                          </Badge>
                          <Badge variant="secondary" className="text-xs">
                            {Math.round(result.score * 100)}% 匹配
                          </Badge>
                        </div>
                        
                        <p className="text-sm text-gray-600 line-clamp-2">
                          {result.content}
                        </p>
                        
                        {result.context && (
                          <p className="text-xs text-purple-600 mt-1 italic">
                            {result.context}
                          </p>
                        )}
                        
                        <div className="flex items-center gap-4 mt-2 text-xs text-gray-500">
                          {result.metadata.lastModified && (
                            <span className="flex items-center gap-1">
                              <Clock className="h-3 w-3" />
                              {formatDate(result.metadata.lastModified, 'short')}
                            </span>
                          )}
                          {result.metadata.author && (
                            <span>作者: {result.metadata.author}</span>
                          )}
                          {result.metadata.tags && result.metadata.tags.length > 0 && (
                            <div className="flex gap-1">
                              {result.metadata.tags.slice(0, 3).map(tag => (
                                <Badge key={tag} variant="secondary" className="text-xs">
                                  {tag}
                                </Badge>
                              ))}
                            </div>
                          )}
                        </div>
                      </div>
                      
                      <Button variant="ghost" size="sm">
                        <ArrowRight className="h-4 w-4" />
                      </Button>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {/* 搜索建议 */}
            {suggestions.length > 0 && results.length === 0 && (
              <div className="p-4">
                <div className="space-y-3">
                  {!query && recentSearches.length > 0 && (
                    <div>
                      <h4 className="text-sm font-medium text-gray-700 mb-2">最近搜索</h4>
                      <div className="space-y-1">
                        {recentSearches.map((search, index) => (
                          <button
                            key={index}
                            onClick={() => handleSearch(search)}
                            className="flex items-center gap-2 w-full p-2 text-left rounded-md hover:bg-gray-50 transition-colors"
                          >
                            <Clock className="h-4 w-4 text-gray-400" />
                            <span className="text-sm text-gray-700">{search}</span>
                          </button>
                        ))}
                      </div>
                    </div>
                  )}

                  <div>
                    <h4 className="text-sm font-medium text-gray-700 mb-2">
                      {query ? '智能建议' : '热门搜索'}
                    </h4>
                    <div className="space-y-1">
                      {suggestions.map((suggestion) => (
                        <button
                          key={suggestion.id}
                          onClick={() => handleSuggestionClick(suggestion)}
                          className="flex items-center gap-2 w-full p-2 text-left rounded-md hover:bg-gray-50 transition-colors"
                        >
                          {suggestion.icon}
                          <span className="text-sm text-gray-700">{suggestion.text}</span>
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              </div>
            )}

            {/* 空状态 */}
            {results.length === 0 && suggestions.length === 0 && query && !isSearching && (
              <div className="p-8 text-center text-gray-500">
                <Search className="h-8 w-8 mx-auto mb-3 text-gray-400" />
                <p>未找到相关结果</p>
                <p className="text-sm mt-1">尝试使用不同的关键词或更改过滤条件</p>
              </div>
            )}
          </div>

          {/* 搜索提示 */}
          <div className="p-3 border-t border-gray-200 bg-gray-50 text-xs text-gray-600">
            💡 提示：您可以问"最近的交易情况如何？"或"哪些客户最活跃？"等自然语言问题
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

// 辅助函数
function getInitialSuggestions(): SearchSuggestion[] {
  return [
    {
      id: 'recent-deals',
      text: '最近的交易情况如何？',
      type: 'query',
      icon: <MessageCircle className="h-4 w-4 text-blue-600" />
    },
    {
      id: 'active-contacts',
      text: '哪些联系人最活跃？',
      type: 'query',
      icon: <Users className="h-4 w-4 text-green-600" />
    },
    {
      id: 'workflow-status',
      text: '工作流状态总览',
      type: 'query',
      icon: <Zap className="h-4 w-4 text-purple-600" />
    },
    {
      id: 'performance-metrics',
      text: '性能指标分析',
      type: 'query',
      icon: <Star className="h-4 w-4 text-orange-600" />
    }
  ]
}

function generateQuerySuggestions(query: string): SearchSuggestion[] {
  const suggestions: SearchSuggestion[] = []
  
  // 基于查询内容生成建议
  if (query.includes('客户') || query.includes('联系')) {
    suggestions.push({
      id: 'contacts-by-name',
      text: `查找包含"${query}"的联系人`,
      type: 'query',
      icon: <Users className="h-4 w-4 text-blue-600" />
    })
  }
  
  if (query.includes('公司') || query.includes('企业')) {
    suggestions.push({
      id: 'companies-by-name',
      text: `查找包含"${query}"的公司`,
      type: 'query',
      icon: <Building className="h-4 w-4 text-green-600" />
    })
  }

  // 添加一些通用建议
  suggestions.push(
    {
      id: 'exact-match',
      text: `"${query}"`,
      type: 'query',
      icon: <Search className="h-4 w-4 text-gray-600" />
    },
    {
      id: 'ai-explain',
      text: `解释"${query}"的含义`,
      type: 'query',
      icon: <Brain className="h-4 w-4 text-purple-600" />
    }
  )

  return suggestions
}

async function parseAISearchResponse(aiContent: string, query: string): Promise<SearchResult[]> {
  // 这里应该解析AI的响应并转换为标准的搜索结果格式
  // 暂时返回模拟数据
  return generateMockResults(query)
}

async function performFallbackSearch(query: string, filter: string): Promise<SearchResult[]> {
  // 模拟搜索延迟
  await new Promise(resolve => setTimeout(resolve, 500))
  return generateMockResults(query)
}

function generateMockResults(query: string): SearchResult[] {
  return [
    {
      id: 'result-1',
      type: 'entity',
      title: `与"${query}"相关的联系人`,
      content: '找到了几个可能相关的联系人记录，包括最近有过互动的客户。',
      score: 0.85,
      metadata: {
        entityType: 'Contact',
        lastModified: new Date(Date.now() - 86400000),
        author: '系统',
        tags: ['客户', '活跃']
      },
      context: 'AI分析了联系记录和互动历史'
    },
    {
      id: 'result-2',
      type: 'insight',
      title: '数据洞察',
      content: `基于"${query}"的分析显示了一些有趣的业务模式和趋势。`,
      score: 0.78,
      metadata: {
        lastModified: new Date(),
        author: 'AI助手',
        tags: ['分析', '趋势']
      },
      context: 'AI生成的智能洞察'
    }
  ]
}
