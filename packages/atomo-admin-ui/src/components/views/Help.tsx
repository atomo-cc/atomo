/**
 * Help / About View — Dashin Documentation & Ecosystem Guide
 */

import { useQuery } from '@tanstack/react-query'
import { apiClient } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '../ui/Card'
import { BookOpen, Github, MessageSquare, LifeBuoy, ExternalLink, HelpCircle } from 'lucide-react'

const LINKS = [
  {
    icon: BookOpen,
    title: 'Documentation',
    description: 'Architecture guides, REST/GraphQL API reference, and TypeScript schema DSL specs.',
    href: 'https://github.com/atomo-cc/atomo/tree/main/docs',
  },
  {
    icon: Github,
    title: 'Source & Issues',
    description: 'Explore the Rust backend core and submit bug reports or PRs on GitHub.',
    href: 'https://github.com/atomo-cc/atomo',
  },
  {
    icon: MessageSquare,
    title: 'Discussions & RFC',
    description: 'Join community discussions, share feedback, and propose architectural RFCs.',
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
    <div className="p-6 space-y-6 max-w-4xl">
      {/* Header */}
      <div className="flex items-center gap-3">
        <div className="w-9 h-9 rounded-bn bg-primary/10 flex items-center justify-center text-primary">
          <HelpCircle className="h-5 w-5" />
        </div>
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-foreground">Help & Resources</h1>
          <p className="text-xs text-icon-muted mt-0.5">
            Atomo Admin is a zero-code dynamic admin framework powered by Dashin Design System.
          </p>
        </div>
      </div>

      {/* Cards grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {LINKS.map((link) => {
          const Icon = link.icon
          return (
            <a
              key={link.title}
              href={link.href}
              target="_blank"
              rel="noreferrer"
              className="group block rounded-bn border border-bn-border bg-content-box p-5 shadow-bn transition-all hover:border-primary/40 hover:shadow-md"
            >
              <div className="flex items-center justify-between mb-2">
                <div className="w-8 h-8 rounded-bn bg-primary/10 flex items-center justify-center text-primary group-hover:bg-primary group-hover:text-white transition-colors">
                  <Icon className="h-4 w-4" />
                </div>
                <ExternalLink className="h-3.5 w-3.5 text-icon-muted opacity-0 group-hover:opacity-100 transition-opacity" />
              </div>
              <h3 className="text-sm font-semibold text-foreground mb-1 group-hover:text-primary transition-colors">
                {link.title}
              </h3>
              <p className="text-xs text-icon-muted leading-relaxed">{link.description}</p>
            </a>
          )
        })}
      </div>

      {/* About runtime card */}
      <Card>
        <CardHeader className="py-4 border-b border-bn-border/60">
          <CardTitle className="flex items-center gap-2">
            <LifeBuoy className="h-4 w-4 text-primary" /> Connected System Runtime
          </CardTitle>
          <CardDescription>Server release and git build metadata</CardDescription>
        </CardHeader>
        <CardContent className="p-5">
          <p className="text-xs text-icon-muted leading-relaxed">
            Running on Atomo server release{' '}
            <span className="font-semibold text-foreground">{version?.version || 'development'}</span>
            {version?.commit && version.commit !== 'unknown' && (
              <>
                {' '}• commit{' '}
                <span className="font-mono font-medium text-foreground">{version.commit.slice(0, 12)}</span>
              </>
            )}
            . See <span className="font-medium text-foreground">Settings</span> for complete network and platform configurations.
          </p>
        </CardContent>
      </Card>
    </div>
  )
}
