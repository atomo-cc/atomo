# Atomo Documentation Deployment Guide

This guide explains how to deploy Atomo's developer documentation to various hosting platforms.

## 🚀 Quick Deployment Options

### Option 1: GitHub Pages (Recommended)
**Free, automatic deployment from your repository**

1. **Enable GitHub Pages**:
   - Go to your repository settings
   - Navigate to "Pages" section
   - Select "GitHub Actions" as source

2. **Automatic Deployment**:
   - The `.github/workflows/docs.yml` workflow will automatically deploy on pushes to `main`
   - Documentation will be available at: `https://your-username.github.io/atomo/`

3. **Custom Domain** (Optional):
   - Add a `CNAME` file in `docs/public/` with your domain
   - Configure DNS to point to GitHub Pages

### Option 2: Netlify
**Easy deployment with preview branches**

1. **Connect Repository**:
   - Sign up at [netlify.com](https://netlify.com)
   - Connect your GitHub repository
   - Set build directory to `docs`

2. **Configuration**:
   - Netlify will automatically detect the `netlify.toml` configuration
   - Build command: `pnpm install && pnpm build`
   - Publish directory: `.vitepress/dist`

3. **Custom Domain**:
   - Add your domain in Netlify dashboard
   - Configure DNS as instructed

### Option 3: Vercel
**Fast global CDN deployment**

1. **Import Project**:
   - Sign up at [vercel.com](https://vercel.com)
   - Import your GitHub repository
   - Vercel will detect the `vercel.json` configuration

2. **Automatic Deployment**:
   - Deploys automatically on every push
   - Preview deployments for pull requests
   - Available at: `https://your-project.vercel.app`

## 🛠️ Local Development

### Prerequisites
```bash
# Ensure you have the required tools
node --version  # v18+
pnpm --version  # v8+
cargo --version # 1.70+
```

### Setup
```bash
# Install dependencies
pnpm install

# Start development server
pnpm dev:docs

# Open browser to http://localhost:5173
```

### Build Process
```bash
# Generate Rust API documentation
pnpm docs:api

# Build documentation site
pnpm build:docs

# Preview production build
cd docs && pnpm preview
```

## 📝 Content Management

### Adding New Pages

1. **Create Markdown File**:
   ```bash
   # Add to appropriate directory
   touch docs/guide/new-feature.md
   ```

2. **Update Navigation**:
   ```typescript
   // docs/.vitepress/config.ts
   sidebar: {
     '/guide/': [
       {
         text: 'Getting Started',
         items: [
           { text: 'New Feature', link: '/guide/new-feature' }
         ]
       }
     ]
   }
   ```

3. **Test Locally**:
   ```bash
   pnpm dev:docs
   ```

### Auto-Generated Content

The documentation includes auto-generated sections:

- **API Reference**: Generated from Rust doc comments
- **CLI Help**: Extracted from clap definitions
- **GraphQL Schema**: Generated from your service schemas

To update auto-generated content:
```bash
# Regenerate API docs
cargo doc --workspace --no-deps --document-private-items

# Rebuild documentation
pnpm build:docs
```

## 🔧 Customization

### Branding
Update the following files for your branding:

```typescript
// docs/.vitepress/config.ts
export default defineConfig({
  title: 'Your Project Name',
  description: 'Your project description',
  themeConfig: {
    logo: '/your-logo.svg',
    // ... other config
  }
})
```

### Custom Styling
Add custom CSS:

```css
/* docs/.vitepress/theme/custom.css */
:root {
  --vp-c-brand: #your-brand-color;
  --vp-c-brand-light: #your-light-color;
  --vp-c-brand-dark: #your-dark-color;
}
```

### Analytics
Add analytics tracking:

```typescript
// docs/.vitepress/config.ts
export default defineConfig({
  head: [
    ['script', { 
      async: '', 
      src: 'https://www.googletagmanager.com/gtag/js?id=GA_MEASUREMENT_ID' 
    }],
    ['script', {}, `
      window.dataLayer = window.dataLayer || [];
      function gtag(){dataLayer.push(arguments);}
      gtag('js', new Date());
      gtag('config', 'GA_MEASUREMENT_ID');
    `]
  ]
})
```

## 🚨 Troubleshooting

### Common Issues

**Build Fails with Rust Errors**:
```bash
# Ensure Rust toolchain is installed
rustup update stable

# Clear cargo cache
cargo clean

# Rebuild documentation
pnpm docs:api
```

**VitePress Build Errors**:
```bash
# Clear node modules and reinstall
rm -rf node_modules pnpm-lock.yaml
pnpm install

# Clear VitePress cache
rm -rf docs/.vitepress/cache
```

**Missing API Documentation**:
```bash
# Ensure cargo doc runs successfully
cargo doc --workspace --no-deps --document-private-items

# Check if target/doc directory exists
ls -la target/doc
```

### Performance Optimization

**Large Documentation Sites**:
- Enable VitePress's [build optimization](https://vitepress.dev/guide/deploy#build-optimization)
- Use dynamic imports for large components
- Optimize images and assets

**Slow Build Times**:
- Use incremental builds in CI
- Cache dependencies properly
- Consider splitting large documentation sections

## 📊 Monitoring

### Analytics Setup
Track documentation usage with:

- **Google Analytics**: Page views, user flow
- **Hotjar**: User behavior and heatmaps  
- **GitHub Insights**: Repository traffic

### Performance Monitoring
Monitor site performance:

- **Lighthouse CI**: Automated performance testing
- **Web Vitals**: Core web vitals tracking
- **Uptime Monitoring**: Site availability

## 🤝 Contributing

### Documentation Standards

1. **Writing Style**:
   - Clear, concise explanations
   - Code examples for all features
   - Step-by-step tutorials
   - Consistent terminology

2. **Markdown Guidelines**:
   - Use proper heading hierarchy
   - Include code syntax highlighting
   - Add alt text for images
   - Link to related sections

3. **Review Process**:
   - Test all code examples
   - Check links and references
   - Verify on multiple devices
   - Get feedback from users

### Automation
The documentation includes automated:

- **Link checking**: Validates all internal/external links
- **Spell checking**: Catches typos and errors
- **Code validation**: Tests all code examples
- **Accessibility**: Ensures WCAG compliance

---

**Ready to deploy your documentation?** Choose your preferred platform above and follow the setup guide! 🚀