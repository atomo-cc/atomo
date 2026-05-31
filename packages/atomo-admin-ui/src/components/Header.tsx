import { Search } from 'lucide-react'

export default function Header() {
  return (
    <header className="h-14 border-b border-gray-200 bg-white px-6">
      <div className="flex h-full items-center justify-between">
        <div>
          <h1 className="text-sm font-semibold text-gray-900">Atomo Admin</h1>
          <p className="text-xs text-gray-500">Schema-driven workspace</p>
        </div>
        <button
          type="button"
          className="inline-flex h-9 w-9 items-center justify-center rounded-md border border-gray-200 text-gray-500 hover:bg-gray-50"
          aria-label="Search"
        >
          <Search className="h-4 w-4" />
        </button>
      </div>
    </header>
  )
}
