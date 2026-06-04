/**
 * Enhanced Dashboard - 增强仪表盘
 * 
 * 业务洞察和数据可视化中心，集成：
 * - 实时业务指标
 * - AI驱动的洞察分析
 * - 交互式数据可视化
 * - 智能推荐和预测
 */

import React, { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { 
  BarChart3, 
  Users, 
  Building, 
  TrendingUp, 
  Activity,
  Plus,
  ArrowUpRight,
  ArrowDownRight,
  Calendar,
  Bell,
  Brain,
  Target,
  Zap,
  Eye,
  Filter,
  RefreshCw,
  Sparkles,
  AlertTriangle,
  CheckCircle,
  Clock,
  DollarSign
} from 'lucide-react'

import { Card, CardHeader, CardTitle, CardContent } from '../ui/Card'
import { Button } from '../ui/Button'
import { Badge } from '../ui/Badge'
import { Progress } from '../ui/Progress'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '../ui/Tabs'
import { useAIAssistant } from '../../lib/ai-assistant'
import { GlobalSearch } from '../ai/GlobalSearch'
import { formatDate, cn } from '../../lib/utils'

interface DashboardStats {
  totalContacts: number
  totalCompanies: number
  totalDeals: number
  totalRevenue: number
  recentActivity: ActivityItem[]
  upcomingTasks: TaskItem[]
  insights: BusinessInsight[]
  trends: TrendData[]
  goals: GoalItem[]
  alerts: AlertItem[]
}

interface ActivityItem {
  id: string
  type: 'contact_created' | 'deal_updated' | 'task_completed' | 'workflow_triggered'
  description: string
  timestamp: Date
  user: string
  entityType?: string
  entityId?: string
}

interface TaskItem {
  id: string
  title: string
  dueDate: Date
  priority: 'high' | 'medium' | 'low'
  assignee: string
  progress: number
  category: string
}

interface BusinessInsight {
  id: string
  type: 'opportunity' | 'warning' | 'trend' | 'prediction'
  title: string
  description: string
  confidence: number
  impact: 'high' | 'medium' | 'low'
  actionable: boolean
  metadata?: Record<string, any>
}

interface TrendData {
  period: string
  contacts: number
  companies: number
  deals: number
  revenue: number
  growth?: {
    contacts: number
    companies: number
    deals: number
    revenue: number
  }
}

interface GoalItem {
  id: string
  title: string
  target: number
  current: number
  period: 'monthly' | 'quarterly' | 'yearly'
  category: string
  deadline: Date
}

interface AlertItem {
  id: string
  type: 'error' | 'warning' | 'info' | 'success'
  title: string
  message: string
  timestamp: Date
  actionRequired: boolean
}

export function EnhancedDashboard() {
  const navigate = useNavigate()
  const { assistant, isAvailable } = useAIAssistant()
  const [selectedTimeRange, setSelectedTimeRange] = useState('30d')
  const [showGlobalSearch, setShowGlobalSearch] = useState(false)
  const [activeTab, setActiveTab] = useState('overview')

  // 获取仪表盘数据
  const { data: stats, isLoading, error, refetch } = useQuery({
    queryKey: ['enhanced-dashboard-stats', selectedTimeRange],
    queryFn: () => fetchDashboardStats(selectedTimeRange),
    refetchInterval: 30000, // 30秒刷新一次
  })

  // AI洞察查询
  const { data: aiInsights, isLoading: aiLoading } = useQuery({
    queryKey: ['ai-insights', selectedTimeRange],
    queryFn: () => generateAIInsights(stats),
    enabled: isAvailable && !!stats,
    refetchInterval: 300000, // 5分钟刷新一次
  })

  const timeRangeOptions = [
    { value: '7d', label: '7天' },
    { value: '30d', label: '30天' },
    { value: '90d', label: '90天' },
    { value: '1y', label: '1年' }
  ]

  if (error) {
    return (
      <div className="p-6">
        <Card className="border-red-200 bg-red-50">
          <CardContent className="py-8 text-center">
            <AlertTriangle className="h-8 w-8 text-red-600 mx-auto mb-3" />
            <p className="text-red-600 font-medium">加载仪表盘数据时出错</p>
            <p className="text-red-500 text-sm mt-1">请检查网络连接或稍后重试</p>
            <Button 
              variant="secondary" 
              className="mt-4"
              onClick={() => refetch()}
            >
              <RefreshCw className="h-4 w-4 mr-2" />
              重新加载
            </Button>
          </CardContent>
        </Card>
      </div>
    )
  }

  if (isLoading) {
    return (
      <div className="p-6">
        <div className="animate-pulse space-y-6">
          <div className="h-8 bg-gray-200 rounded w-1/4"></div>
          <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
            {[...Array(4)].map((_, i) => (
              <div key={i} className="h-32 bg-gray-200 rounded"></div>
            ))}
          </div>
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
            <div className="h-80 bg-gray-200 rounded"></div>
            <div className="h-80 bg-gray-200 rounded"></div>
            <div className="h-80 bg-gray-200 rounded"></div>
          </div>
        </div>
      </div>
    )
  }

  const getInsightIcon = (type: BusinessInsight['type']) => {
    switch (type) {
      case 'opportunity': return <Target className="h-4 w-4 text-green-600" />
      case 'warning': return <AlertTriangle className="h-4 w-4 text-yellow-600" />
      case 'trend': return <TrendingUp className="h-4 w-4 text-blue-600" />
      case 'prediction': return <Brain className="h-4 w-4 text-purple-600" />
      default: return <Eye className="h-4 w-4 text-gray-600" />
    }
  }

  const getAlertIcon = (type: AlertItem['type']) => {
    switch (type) {
      case 'error': return <AlertTriangle className="h-4 w-4 text-red-600" />
      case 'warning': return <AlertTriangle className="h-4 w-4 text-yellow-600" />
      case 'success': return <CheckCircle className="h-4 w-4 text-green-600" />
      default: return <Bell className="h-4 w-4 text-blue-600" />
    }
  }

  return (
    <div className="p-6 space-y-6">
      {/* 页面标题和控制 */}
      <div className="flex justify-between items-center">
        <div>
          <h1 className="text-3xl font-bold text-gray-900 flex items-center gap-3">
            <BarChart3 className="h-8 w-8 text-primary-600" />
            业务仪表盘
          </h1>
          <p className="text-gray-600 mt-1">实时业务洞察和数据可视化中心</p>
        </div>
        <div className="flex items-center gap-3">
          {/* 时间范围选择 */}
          <select
            value={selectedTimeRange}
            onChange={(e) => setSelectedTimeRange(e.target.value)}
            className="px-3 py-2 border border-gray-300 rounded-md text-sm"
          >
            {timeRangeOptions.map(option => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>

          <Button variant="secondary" onClick={() => setShowGlobalSearch(true)}>
            <Brain className="h-4 w-4 mr-2" />
            智能搜索
          </Button>
          
          <Button variant="secondary" onClick={() => refetch()}>
            <RefreshCw className="h-4 w-4 mr-2" />
            刷新
          </Button>
        </div>
      </div>

      {/* 关键指标卡片 */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-6">
        <MetricCard
          title="联系人总数"
          value={stats?.totalContacts || 0}
          growth={12}
          icon={<Users className="h-8 w-8 text-blue-600" />}
          color="blue"
          onClick={() => navigate('/entities/Contact')}
        />
        
        <MetricCard
          title="公司总数"
          value={stats?.totalCompanies || 0}
          growth={8}
          icon={<Building className="h-8 w-8 text-green-600" />}
          color="green"
          onClick={() => navigate('/entities/Company')}
        />
        
        <MetricCard
          title="交易总数"
          value={stats?.totalDeals || 0}
          growth={15}
          icon={<Target className="h-8 w-8 text-purple-600" />}
          color="purple"
          onClick={() => navigate('/entities/Deal')}
        />
        
        <MetricCard
          title="总收入"
          value={stats?.totalRevenue || 0}
          growth={23}
          icon={<DollarSign className="h-8 w-8 text-orange-600" />}
          color="orange"
          prefix="¥"
          onClick={() => navigate('/analytics/revenue')}
        />
      </div>

      {/* 系统状态警报 */}
      {stats?.alerts && stats.alerts.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {stats.alerts.slice(0, 2).map((alert) => (
            <Card key={alert.id} className={cn(
              "border-l-4",
              alert.type === 'error' && "border-l-red-500 bg-red-50",
              alert.type === 'warning' && "border-l-yellow-500 bg-yellow-50",
              alert.type === 'success' && "border-l-green-500 bg-green-50",
              alert.type === 'info' && "border-l-blue-500 bg-blue-50"
            )}>
              <CardContent className="p-4">
                <div className="flex items-center gap-3">
                  {getAlertIcon(alert.type)}
                  <div className="flex-1">
                    <h4 className="font-medium text-gray-900">{alert.title}</h4>
                    <p className="text-sm text-gray-600 mt-1">{alert.message}</p>
                    <p className="text-xs text-gray-500 mt-1">
                      {formatDate(alert.timestamp, 'time')}
                    </p>
                  </div>
                  {alert.actionRequired && (
                    <Button size="sm" variant="secondary">
                      处理
                    </Button>
                  )}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      {/* 标签页内容 */}
      <Tabs value={activeTab} onValueChange={setActiveTab} className="space-y-6">
        <TabsList className="grid w-full grid-cols-4">
          <TabsTrigger value="overview">业务概览</TabsTrigger>
          <TabsTrigger value="insights">AI洞察</TabsTrigger>
          <TabsTrigger value="goals">目标进度</TabsTrigger>
          <TabsTrigger value="activity">活动监控</TabsTrigger>
        </TabsList>

        <TabsContent value="overview" className="space-y-6">
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
            {/* 最近活动 */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Activity className="h-5 w-5" />
                  最近活动
                </CardTitle>
              </CardHeader>
              <CardContent>
                <ActivityFeed activities={stats?.recentActivity || []} />
              </CardContent>
            </Card>

            {/* 待办任务 */}
            <Card>
              <CardHeader>
                <CardTitle className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <Calendar className="h-5 w-5" />
                    待办任务
                  </div>
                  <Button variant="ghost" size="sm">
                    <Plus className="h-4 w-4" />
                  </Button>
                </CardTitle>
              </CardHeader>
              <CardContent>
                <TaskList tasks={stats?.upcomingTasks || []} />
              </CardContent>
            </Card>
          </div>
        </TabsContent>

        <TabsContent value="insights" className="space-y-6">
          <InsightPanel 
            insights={stats?.insights || []} 
            aiInsights={aiInsights}
            isLoading={aiLoading}
          />
        </TabsContent>

        <TabsContent value="goals" className="space-y-6">
          <GoalTracker goals={stats?.goals || []} />
        </TabsContent>

        <TabsContent value="activity" className="space-y-6">
          <ActivityDashboard 
            activities={stats?.recentActivity || []}
            trends={stats?.trends || []}
          />
        </TabsContent>
      </Tabs>

      {/* 全局搜索组件 */}
      <GlobalSearch 
        isOpen={showGlobalSearch}
        onClose={() => setShowGlobalSearch(false)}
      />
    </div>
  )
}

// 指标卡片组件
interface MetricCardProps {
  title: string
  value: number
  growth: number
  icon: React.ReactNode
  color: string
  prefix?: string
  onClick?: () => void
}

function MetricCard({ title, value, growth, icon, color, prefix = '', onClick }: MetricCardProps) {
  const isPositive = growth > 0
  
  return (
    <Card 
      className={cn(
        "hover:shadow-md transition-shadow cursor-pointer",
        onClick && "hover:border-primary-300"
      )}
      onClick={onClick}
    >
      <CardContent className="p-6">
        <div className="flex items-center justify-between">
          <div>
            <p className="text-sm font-medium text-gray-600">{title}</p>
            <p className="text-3xl font-bold text-gray-900">
              {prefix}{(value ?? 0).toLocaleString()}
            </p>
          </div>
          {icon}
        </div>
        <div className="flex items-center mt-4 text-sm">
          {isPositive ? (
            <ArrowUpRight className="h-4 w-4 text-green-600 mr-1" />
          ) : (
            <ArrowDownRight className="h-4 w-4 text-red-600 mr-1" />
          )}
          <span className={isPositive ? "text-green-600" : "text-red-600"}>
            {isPositive ? '+' : ''}{growth}%
          </span>
          <span className="text-gray-500 ml-1">本月</span>
        </div>
      </CardContent>
    </Card>
  )
}

// 活动流组件
function ActivityFeed({ activities }: { activities: ActivityItem[] }) {
  if (activities.length === 0) {
    return (
      <div className="text-center py-8 text-gray-500">
        <Activity className="h-8 w-8 mx-auto mb-3 text-gray-400" />
        <p>暂无最近活动</p>
      </div>
    )
  }

  return (
    <div className="space-y-4 max-h-64 overflow-y-auto">
      {activities.map((activity) => (
        <div key={activity.id} className="flex items-start gap-3 p-3 hover:bg-gray-50 rounded-md transition-colors">
          <div className="w-2 h-2 bg-blue-600 rounded-full mt-2 flex-shrink-0" />
          <div className="flex-1">
            <p className="text-sm text-gray-900">{activity.description}</p>
            <div className="flex items-center gap-2 mt-1">
              <span className="text-xs text-gray-500">{activity.user}</span>
              <span className="text-xs text-gray-400">•</span>
              <span className="text-xs text-gray-500">
                {formatDate(activity.timestamp, 'time')}
              </span>
              {activity.entityType && (
                <>
                  <span className="text-xs text-gray-400">•</span>
                  <Badge variant="secondary" className="text-xs">
                    {activity.entityType}
                  </Badge>
                </>
              )}
            </div>
          </div>
        </div>
      ))}
    </div>
  )
}

// 任务列表组件
function TaskList({ tasks }: { tasks: TaskItem[] }) {
  if (tasks.length === 0) {
    return (
      <div className="text-center py-8 text-gray-500">
        <Calendar className="h-8 w-8 mx-auto mb-3 text-gray-400" />
        <p>暂无待办任务</p>
      </div>
    )
  }

  return (
    <div className="space-y-3 max-h-64 overflow-y-auto">
      {tasks.map((task) => (
        <div key={task.id} className="p-3 border border-gray-200 rounded-md hover:bg-gray-50 transition-colors">
          <div className="flex items-start gap-3">
            <input 
              type="checkbox" 
              className="mt-1 rounded border-gray-300" 
              onChange={() => {/* 处理任务完成 */}} 
            />
            <div className="flex-1">
              <p className="text-sm font-medium text-gray-900">{task.title}</p>
              <div className="flex items-center gap-2 mt-1 mb-2">
                <span className="text-xs text-gray-500">
                  {formatDate(task.dueDate, 'short')}
                </span>
                <Badge 
                  variant={task.priority === 'high' ? 'danger' : 
                          task.priority === 'medium' ? 'warning' : 'secondary'}
                  className="text-xs"
                >
                  {task.priority === 'high' ? '高' : 
                   task.priority === 'medium' ? '中' : '低'}
                </Badge>
                <span className="text-xs text-gray-500">{task.assignee}</span>
              </div>
              {task.progress > 0 && (
                <div className="flex items-center gap-2">
                  <Progress value={task.progress} className="h-2 flex-1" />
                  <span className="text-xs text-gray-500">{task.progress}%</span>
                </div>
              )}
            </div>
          </div>
        </div>
      ))}
    </div>
  )
}

// 洞察面板组件
function InsightPanel({ insights, aiInsights, isLoading }: {
  insights: BusinessInsight[]
  aiInsights?: any
  isLoading: boolean
}) {
  const getInsightIcon = (type: BusinessInsight['type']) => {
    switch (type) {
      case 'opportunity': return <Target className="h-4 w-4 text-green-600" />
      case 'warning': return <AlertTriangle className="h-4 w-4 text-yellow-600" />
      case 'trend': return <TrendingUp className="h-4 w-4 text-blue-600" />
      case 'prediction': return <Brain className="h-4 w-4 text-purple-600" />
      default: return <Eye className="h-4 w-4 text-gray-600" />
    }
  }

  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Brain className="h-5 w-5 text-purple-600" />
            AI业务洞察
            {isLoading && (
              <div className="animate-spin rounded-full h-4 w-4 border-2 border-purple-600 border-t-transparent" />
            )}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-4">
            {insights.map((insight) => (
              <div key={insight.id} className="p-4 border border-gray-200 rounded-md">
                <div className="flex items-start gap-3">
                  {getInsightIcon(insight.type)}
                  <div className="flex-1">
                    <h4 className="font-medium text-gray-900">{insight.title}</h4>
                    <p className="text-sm text-gray-600 mt-1">{insight.description}</p>
                    <div className="flex items-center gap-2 mt-2">
                      <Badge variant="secondary" className="text-xs">
                        {Math.round(insight.confidence * 100)}% 置信度
                      </Badge>
                      <Badge 
                        variant={insight.impact === 'high' ? 'danger' : 
                                insight.impact === 'medium' ? 'warning' : 'secondary'}
                        className="text-xs"
                      >
                        {insight.impact === 'high' ? '高影响' : 
                         insight.impact === 'medium' ? '中影响' : '低影响'}
                      </Badge>
                      {insight.actionable && (
                        <Badge variant="success" className="text-xs">
                          可操作
                        </Badge>
                      )}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Sparkles className="h-5 w-5 text-yellow-600" />
            智能推荐
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="space-y-3">
            <div className="p-3 bg-blue-50 border border-blue-200 rounded-md">
              <p className="text-sm font-medium text-blue-900">建议关注</p>
              <p className="text-sm text-blue-700 mt-1">
                有3个高价值潜在客户在过去7天内活跃度较高，建议优先跟进
              </p>
            </div>
            <div className="p-3 bg-green-50 border border-green-200 rounded-md">
              <p className="text-sm font-medium text-green-900">机会发现</p>
              <p className="text-sm text-green-700 mt-1">
                科技行业的成交率比平均水平高18%，可考虑增加相关投入
              </p>
            </div>
            <div className="p-3 bg-yellow-50 border border-yellow-200 rounded-md">
              <p className="text-sm font-medium text-yellow-900">风险提醒</p>
              <p className="text-sm text-yellow-700 mt-1">
                本月有5个大额交易即将到期，需要加强跟进力度
              </p>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  )
}

// 目标追踪组件
function GoalTracker({ goals }: { goals: GoalItem[] }) {
  return (
    <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
      {goals.map((goal) => {
        const progress = Math.min((goal.current / goal.target) * 100, 100)
        const isOverdue = new Date() > goal.deadline
        const daysLeft = Math.ceil((goal.deadline.getTime() - Date.now()) / (1000 * 60 * 60 * 24))
        
        return (
          <Card key={goal.id}>
            <CardContent className="p-6">
              <div className="space-y-4">
                <div>
                  <h3 className="font-medium text-gray-900">{goal.title}</h3>
                  <p className="text-sm text-gray-500">{goal.category}</p>
                </div>
                
                <div className="space-y-2">
                  <div className="flex justify-between text-sm">
                    <span>进度</span>
                    <span className="font-medium">{progress.toFixed(1)}%</span>
                  </div>
                  <Progress value={progress} className="h-3" />
                  <div className="flex justify-between text-xs text-gray-500">
                    <span>{(goal.current ?? 0).toLocaleString()}</span>
                    <span>{(goal.target ?? 0).toLocaleString()}</span>
                  </div>
                </div>
                
                <div className="flex items-center justify-between text-sm">
                  <span className="text-gray-500">
                    {isOverdue ? '已过期' : `${daysLeft}天剩余`}
                  </span>
                  <Badge 
                    variant={progress >= 100 ? 'success' : 
                            progress >= 75 ? 'secondary' : 
                            isOverdue ? 'danger' : 'warning'}
                  >
                    {progress >= 100 ? '已完成' : 
                     progress >= 75 ? '接近完成' : 
                     isOverdue ? '逾期' : '进行中'}
                  </Badge>
                </div>
              </div>
            </CardContent>
          </Card>
        )
      })}
    </div>
  )
}

// 活动仪表盘组件
function ActivityDashboard({ activities, trends }: { 
  activities: ActivityItem[]
  trends: TrendData[] 
}) {
  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle>活动趋势</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="h-64 flex items-center justify-center text-gray-500">
            <div className="text-center">
              <BarChart3 className="h-8 w-8 mx-auto mb-2 text-gray-400" />
              <p>活动趋势图表将在集成图表库后显示</p>
            </div>
          </div>
        </CardContent>
      </Card>
      
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        <Card>
          <CardHeader>
            <CardTitle>实时活动流</CardTitle>
          </CardHeader>
          <CardContent>
            <ActivityFeed activities={activities} />
          </CardContent>
        </Card>
        
        <Card>
          <CardHeader>
            <CardTitle>活动统计</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="space-y-4">
              <div className="flex justify-between items-center">
                <span className="text-sm text-gray-600">今日活动</span>
                <span className="font-semibold">
                  {activities.filter(a => 
                    new Date(a.timestamp).toDateString() === new Date().toDateString()
                  ).length}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-gray-600">本周活动</span>
                <span className="font-semibold">
                  {activities.filter(a => 
                    Date.now() - a.timestamp.getTime() < 7 * 24 * 60 * 60 * 1000
                  ).length}
                </span>
              </div>
              <div className="flex justify-between items-center">
                <span className="text-sm text-gray-600">最活跃用户</span>
                <span className="font-semibold">
                  {activities.length > 0 ? activities[0].user : '暂无'}
                </span>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  )
}

// 模拟API函数
async function fetchDashboardStats(timeRange: string): Promise<DashboardStats> {
  // 模拟API延迟
  await new Promise(resolve => setTimeout(resolve, 1000))
  
  return {
    totalContacts: 1247,
    totalCompanies: 328,
    totalDeals: 89,
    totalRevenue: 2840000,
    recentActivity: [
      {
        id: '1',
        type: 'contact_created',
        description: '新建联系人：张三',
        timestamp: new Date(Date.now() - 300000),
        user: '李四',
        entityType: 'Contact',
        entityId: 'contact-123'
      },
      {
        id: '2',
        type: 'deal_updated',
        description: '更新交易：软件采购项目',
        timestamp: new Date(Date.now() - 600000),
        user: '王五',
        entityType: 'Deal',
        entityId: 'deal-456'
      },
      {
        id: '3',
        type: 'workflow_triggered',
        description: '触发工作流：客户欢迎邮件',
        timestamp: new Date(Date.now() - 900000),
        user: '系统',
        entityType: 'Workflow',
        entityId: 'wf-welcome'
      }
    ],
    upcomingTasks: [
      {
        id: '1',
        title: '准备客户演示PPT',
        dueDate: new Date(Date.now() + 86400000),
        priority: 'high',
        assignee: '张三',
        progress: 60,
        category: '销售'
      },
      {
        id: '2',
        title: '跟进潜在客户',
        dueDate: new Date(Date.now() + 172800000),
        priority: 'medium',
        assignee: '李四',
        progress: 30,
        category: '市场'
      }
    ],
    insights: [
      {
        id: '1',
        type: 'opportunity',
        title: '销售机会发现',
        description: '科技行业客户的转化率比平均水平高25%，建议加大投入',
        confidence: 0.87,
        impact: 'high',
        actionable: true
      },
      {
        id: '2',
        type: 'trend',
        title: '增长趋势分析',
        description: '企业客户数量持续增长，预计下季度将突破400家',
        confidence: 0.92,
        impact: 'medium',
        actionable: false
      }
    ],
    trends: [],
    goals: [
      {
        id: '1',
        title: '月度新客户目标',
        target: 50,
        current: 38,
        period: 'monthly',
        category: '销售',
        deadline: new Date(Date.now() + 15 * 24 * 60 * 60 * 1000)
      },
      {
        id: '2',
        title: '季度收入目标',
        target: 1000000,
        current: 750000,
        period: 'quarterly',
        category: '财务',
        deadline: new Date(Date.now() + 45 * 24 * 60 * 60 * 1000)
      }
    ],
    alerts: [
      {
        id: '1',
        type: 'warning',
        title: '系统性能警告',
        message: 'API响应时间较平时增加了15%，建议检查服务器状态',
        timestamp: new Date(Date.now() - 1800000),
        actionRequired: true
      }
    ]
  }
}

async function generateAIInsights(stats: any) {
  // 模拟AI分析延迟
  await new Promise(resolve => setTimeout(resolve, 2000))
  
  return {
    summary: '基于最近30天的数据分析，您的业务呈现健康增长态势',
    recommendations: [
      '建议重点关注科技行业客户，转化率显著高于平均水平',
      '考虑优化销售流程，缩短平均成交周期',
      '增加客户满意度调研，提升客户留存率'
    ]
  }
}
