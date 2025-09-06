# Atomo Documentation

This directory contains the complete developer documentation for Atomo, built with VitePress.

## Development

```bash
# From repo root (recommended)
pnpm docs:serve      # Start VitePress dev server
pnpm docs:api        # Build Rust docs into docs/.vitepress/dist/api

# Or from docs/ directly
pnpm install         # Install docs deps
pnpm dev             # Start dev server
pnpm build           # Build static site
pnpm preview         # Preview production build
```

## Structure

```
docs/
├── .vitepress/
│   ├── config.ts          # VitePress configuration
│   └── theme/             # Custom theme (if needed)
├── guide/                 # User guides and tutorials
│   ├── getting-started.md
│   ├── introduction.md
│   └── ...
├── api/                   # API reference documentation
│   ├── index.md
│   ├── cli.md
│   └── ...
├── examples/              # Code examples and use cases
└── index.md              # Homepage
```

## Deployment

The documentation is automatically deployed when changes are pushed to the main branch:

- **GitHub Pages**: https://atomo-org.github.io/atomo/
- **Netlify**: https://atomo-docs.netlify.app/
- **Vercel**: https://atomo-docs.vercel.app/

## Contributing

To contribute to the documentation:

1. Edit the relevant `.md` files
2. Test locally with `pnpm dev`
3. Submit a pull request

The documentation uses VitePress with:
- **Markdown** for content
- **Vue components** for interactive elements
- **TypeScript** for configuration
 - **Auto-generated** API docs from Rust code
