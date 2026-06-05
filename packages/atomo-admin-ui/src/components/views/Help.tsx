/**
 * Help / About View
 *
 * Replaces the old dead `/help` link (which silently fell back to the dashboard)
 * with a real page: what this admin is, the running server build, and where to get
 * documentation and support.
 */

import { useQuery } from '@tanstack/react-query'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { BookOpen, Github, MessageSquare, LifeBuoy } from 'lucide-react'

const LINKS = [
  {
    icon: BookOpen,
    title: 'Documentation',
    description: 'Guides, API reference, and the schema DSL.',
    href: 'https://github.com/atomo-cc/atomo/tree/main/docs',
  },
  {
    icon: Github,
    title: 'Source & issues',
    description: 'Report a bug or request a feature on GitHub.',
    href: 'https://github.com/atomo-cc/atomo',
  },
  {
    icon: MessageSquare,
    title: 'Discussions',
    description: 'Ask questions and share feedback.',
    href: 'https://github.com/atomo-cc/atomo/discussions',
  },
]

export function Help() {
  const { data: version } = useQuery({
    queryKey: ['server-version'],
    queryFn: () => apiClient.getVersion(),
    staleTime: 5 * 60_000,
    retry: false,
  })

  return (
    <div className="p-6 space-y-6 max-w-3xl">
      <div>
        <h1 className="text-3xl font-bold text-gray-900">Help</h1>
        <p className="text-gray-600 mt-2">
          Atomo Admin is a schema-driven UI for your Atomo backend — it introspects the
          server’s models and generates the browse, create, and edit views you see here.
        </p>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {LINKS.map((link) => {
          const Icon = link.icon
          return (
            <a
              key={link.title}
              href={link.href}
              target="_blank"
              rel="noreferrer"
              className="block rounded-lg border border-gray-200 bg-white p-4 transition-shadow hover:shadow-md"
            >
              <div className="flex items-center gap-2 mb-1">
                <Icon className="h-5 w-5 text-primary-600" />
                <span className="font-medium text-gray-900">{link.title}</span>
              </div>
              <p className="text-sm text-gray-600">{link.description}</p>
            </a>
          )
        })}
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <LifeBuoy className="h-5 w-5" /> About
          </CardTitle>
          <CardDescription>The Atomo server this admin is connected to.</CardDescription>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-gray-600">
            Server version{' '}
            <span className="font-medium text-gray-900">{version?.version || 'unknown'}</span>
            {version?.commit && version.commit !== 'unknown' && (
              <>
                {' '}· commit{' '}
                <span className="font-mono text-gray-900">{version.commit.slice(0, 12)}</span>
              </>
            )}
            . See <span className="font-medium">Settings</span> for full build details.
          </p>
        </CardContent>
      </Card>
    </div>
  )
}
