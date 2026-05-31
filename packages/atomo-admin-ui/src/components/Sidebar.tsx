import { BarChart3, Database, GitBranch, Settings } from 'lucide-react'

const navItems = [
  { label: 'Dashboard', icon: BarChart3 },
  { label: 'Entities', icon: Database },
  { label: 'Workflows', icon: GitBranch },
  { label: 'Settings', icon: Settings },
]

export default function Sidebar() {
  return (
    <aside className="fixed inset-y-0 left-0 hidden w-64 border-r border-gray-200 bg-white lg:block">
      <div className="flex h-14 items-center border-b border-gray-200 px-6">
        <span className="text-sm font-semibold text-gray-900">Atomo</span>
      </div>
      <nav className="space-y-1 p-3">
        {navItems.map((item) => (
          <button
            key={item.label}
            type="button"
            className="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm text-gray-700 hover:bg-gray-100"
          >
            <item.icon className="h-4 w-4 text-gray-500" />
            {item.label}
          </button>
        ))}
      </nav>
    </aside>
  )
}
