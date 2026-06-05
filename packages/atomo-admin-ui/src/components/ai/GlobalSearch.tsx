/**
 * Global Search - Global intelligent search component
 *
 * LLM-powered search across the entire knowledge base, providing:
 * - Natural language queries
 * - Intelligent cross-model search
 * - Context-aware results
 * - Q&A-style interaction
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

  // Search filters
  const searchFilters = [
    { id: 'all', label: 'All', icon: <Search className="h-4 w-4" /> },
    { id: 'contacts', label: 'Contacts', icon: <Users className="h-4 w-4" /> },
    { id: 'companies', label: 'Companies', icon: <Building className="h-4 w-4" /> },
    { id: 'documents', label: 'Documents', icon: <FileText className="h-4 w-4" /> },
    { id: 'workflows', label: 'Workflows', icon: <Zap className="h-4 w-4" /> }
  ]

  // Focus the input
  useEffect(() => {
    if (isOpen && inputRef.current) {
      inputRef.current.focus()
    }
  }, [isOpen])

  // Initialize suggestions
  useEffect(() => {
    if (isOpen && !query) {
      setSuggestions(getInitialSuggestions())
    }
  }, [isOpen, query])

  // Load recent searches
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
      // Save to recent searches
      const updatedRecent = [finalQuery, ...recentSearches.filter(q => q !== finalQuery)].slice(0, 5)
      setRecentSearches(updatedRecent)
      localStorage.setItem('recent-searches', JSON.stringify(updatedRecent))

      if (isAvailable) {
        // Use AI-powered search
        const aiResponse = await assistant.intelligentSearch(
          finalQuery,
          selectedFilter === 'all' ? undefined : [selectedFilter]
        )

        // Parse the AI response and convert it into search results
        const results = await parseAISearchResponse(aiResponse.content, finalQuery)
        setResults(results)
      } else {
        // Fallback search
        const results = await performFallbackSearch(finalQuery, selectedFilter)
        setResults(results)
      }

      setSuggestions([])
    } catch (error) {
      console.error('Search failed:', error)
      // Show an error state or fall back to the fallback search
      const fallbackResults = await performFallbackSearch(finalQuery, selectedFilter)
      setResults(fallbackResults)
    } finally {
      setIsSearching(false)
    }
  }

  const handleQueryChange = (value: string) => {
    setQuery(value)

    // Generate suggestions dynamically
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
      entity: 'Entity',
      document: 'Document',
      workflow: 'Workflow',
      user: 'User',
      insight: 'Insight'
    }
    return labels[type] || 'Unknown'
  }

  if (!isOpen) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center bg-black bg-opacity-50 pt-20">
      <Card className="w-full max-w-2xl max-h-[80vh] overflow-hidden">
        <CardContent className="p-0">
          {/* Search header */}
          <div className="p-4 border-b border-gray-200">
            <div className="flex items-center gap-3">
              <div className="flex-1 relative">
                <Brain className="absolute left-3 top-1/2 transform -translate-y-1/2 h-5 w-5 text-purple-600" />
                <Input
                  ref={inputRef}
                  value={query}
                  onChange={(e) => handleQueryChange(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder="Smart search: ask anything about your business..."
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

            {/* Filters */}
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

          {/* Search content */}
          <div className="max-h-96 overflow-y-auto">
            {/* Search results */}
            {results.length > 0 && (
              <div className="p-4 space-y-3">
                <div className="flex items-center gap-2 text-sm text-gray-600">
                  <Sparkles className="h-4 w-4" />
                  Found {results.length} matching results
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
                            {Math.round(result.score * 100)}% match
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
                            <span>Author: {result.metadata.author}</span>
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

            {/* Search suggestions */}
            {suggestions.length > 0 && results.length === 0 && (
              <div className="p-4">
                <div className="space-y-3">
                  {!query && recentSearches.length > 0 && (
                    <div>
                      <h4 className="text-sm font-medium text-gray-700 mb-2">Recent Searches</h4>
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
                      {query ? 'Smart Suggestions' : 'Popular Searches'}
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

            {/* Empty state */}
            {results.length === 0 && suggestions.length === 0 && query && !isSearching && (
              <div className="p-8 text-center text-gray-500">
                <Search className="h-8 w-8 mx-auto mb-3 text-gray-400" />
                <p>No matching results found</p>
                <p className="text-sm mt-1">Try different keywords or adjust your filters</p>
              </div>
            )}
          </div>

          {/* Search tip */}
          <div className="p-3 border-t border-gray-200 bg-gray-50 text-xs text-gray-600">
            💡 Tip: Try natural-language questions like "How are recent deals doing?" or "Which customers are most active?"
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

// Helper functions
function getInitialSuggestions(): SearchSuggestion[] {
  return [
    {
      id: 'recent-deals',
      text: 'How are recent deals doing?',
      type: 'query',
      icon: <MessageCircle className="h-4 w-4 text-blue-600" />
    },
    {
      id: 'active-contacts',
      text: 'Which contacts are most active?',
      type: 'query',
      icon: <Users className="h-4 w-4 text-green-600" />
    },
    {
      id: 'workflow-status',
      text: 'Workflow status overview',
      type: 'query',
      icon: <Zap className="h-4 w-4 text-purple-600" />
    },
    {
      id: 'performance-metrics',
      text: 'Performance metrics analysis',
      type: 'query',
      icon: <Star className="h-4 w-4 text-orange-600" />
    }
  ]
}

function generateQuerySuggestions(query: string): SearchSuggestion[] {
  const suggestions: SearchSuggestion[] = []

  // Generate suggestions based on the query content
  if (query.includes('customer') || query.includes('contact')) {
    suggestions.push({
      id: 'contacts-by-name',
      text: `Find contacts containing "${query}"`,
      type: 'query',
      icon: <Users className="h-4 w-4 text-blue-600" />
    })
  }

  if (query.includes('company') || query.includes('business')) {
    suggestions.push({
      id: 'companies-by-name',
      text: `Find companies containing "${query}"`,
      type: 'query',
      icon: <Building className="h-4 w-4 text-green-600" />
    })
  }

  // Add some generic suggestions
  suggestions.push(
    {
      id: 'exact-match',
      text: `"${query}"`,
      type: 'query',
      icon: <Search className="h-4 w-4 text-gray-600" />
    },
    {
      id: 'ai-explain',
      text: `Explain what "${query}" means`,
      type: 'query',
      icon: <Brain className="h-4 w-4 text-purple-600" />
    }
  )

  return suggestions
}

async function parseAISearchResponse(aiContent: string, query: string): Promise<SearchResult[]> {
  // This should parse the AI response and convert it into the standard search result format
  // For now, return mock data
  return generateMockResults(query)
}

async function performFallbackSearch(query: string, filter: string): Promise<SearchResult[]> {
  // Simulate search latency
  await new Promise(resolve => setTimeout(resolve, 500))
  return generateMockResults(query)
}

function generateMockResults(query: string): SearchResult[] {
  return [
    {
      id: 'result-1',
      type: 'entity',
      title: `Contacts related to "${query}"`,
      content: 'Found several potentially relevant contact records, including customers you have interacted with recently.',
      score: 0.85,
      metadata: {
        entityType: 'Contact',
        lastModified: new Date(Date.now() - 86400000),
        author: 'System',
        tags: ['Customer', 'Active']
      },
      context: 'AI analyzed contact records and interaction history'
    },
    {
      id: 'result-2',
      type: 'insight',
      title: 'Data Insight',
      content: `Analysis based on "${query}" reveals some interesting business patterns and trends.`,
      score: 0.78,
      metadata: {
        lastModified: new Date(),
        author: 'AI Assistant',
        tags: ['Analysis', 'Trends']
      },
      context: 'AI-generated intelligent insight'
    }
  ]
}
