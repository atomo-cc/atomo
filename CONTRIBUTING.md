# 贡献指南

欢迎参与 Atomo Content Core 的开发！本指南将帮助您了解如何为项目做出贡献。

## 行为准则

请确保您在参与项目时遵循我们的 [行为准则](CODE_OF_CONDUCT.md)。我们致力于营造一个开放、包容的社区环境。

## 如何贡献

### 报告问题

1. **搜索现有问题**: 首先检查是否已有类似问题
2. **创建详细报告**: 使用 issue 模板，提供复现步骤
3. **提供环境信息**: 操作系统、Rust 版本、Node.js 版本等

### 提交功能请求

1. **使用功能请求模板**
2. **详细描述用例**
3. **解释为什么需要此功能**
4. **提供设计建议**（如果有）

### 代码贡献

#### 前置要求

- Rust 1.70+
- Node.js 18+
- pnpm 8+
- Git

#### 开发流程

1. **Fork 仓库**
   ```bash
   git clone https://github.com/your-username/atomo.git
   cd atomo
   ```

2. **创建功能分支**
   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **安装依赖**
   ```bash
   cargo build
   pnpm install
   ```

4. **进行开发**
   - 编写代码
   - 添加测试
   - 更新文档

5. **运行测试**
   ```bash
   cargo test --all
   cargo clippy --all-targets --all-features
   cargo fmt --all -- --check
   pnpm test
   ```

6. **提交更改**
   ```bash
   git add .
   git commit -m "feat: add your feature description"
   ```

7. **推送并创建 PR**
   ```bash
   git push origin feature/your-feature-name
   ```

#### 提交消息规范

使用 [Conventional Commits](https://conventionalcommits.org/) 格式：

```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

**类型 (type):**
- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档更新
- `style`: 代码格式化
- `refactor`: 重构
- `test`: 测试相关
- `chore`: 构建或工具变更

**示例:**
```
feat(cli): add project initialization command

Add `atomo init` command to bootstrap new projects with templates.

Closes #123
```

## 代码规范

### Rust 代码

- 使用 `cargo fmt` 格式化代码
- 使用 `cargo clippy` 检查代码质量
- 遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- 编写有意义的错误消息
- 添加文档注释

**示例:**
```rust
/// Creates a new contact with the given information.
/// 
/// # Arguments
/// 
/// * `name` - The contact's full name
/// * `email` - The contact's email address
/// 
/// # Returns
/// 
/// Returns a `Result` containing the created `Contact` or an error.
/// 
/// # Errors
/// 
/// Returns an error if the email format is invalid.
pub async fn create_contact(name: String, email: String) -> Result<Contact, ContactError> {
    // 实现
}
```

### TypeScript 代码

- 使用 Prettier 格式化
- 遵循 ESLint 规则
- 使用严格的 TypeScript 配置
- 编写 JSDoc 注释

**示例:**
```typescript
/**
 * Represents a contact in the CRM system.
 */
export interface Contact {
  /** Unique identifier for the contact */
  id: string;
  /** Full name of the contact */
  name: string;
  /** Email address of the contact */
  email: string;
  /** Associated company (optional) */
  company?: Company;
}
```

### 测试

- 为新功能编写单元测试
- 为重要功能编写集成测试
- 确保测试覆盖率不下降
- 使用描述性的测试名称

**Rust 测试示例:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_contact_with_valid_email() {
        // 测试实现
    }

    #[test]
    fn test_create_contact_with_invalid_email_returns_error() {
        // 测试实现
    }
}
```

**TypeScript 测试示例:**
```typescript
describe('ContactService', () => {
  it('should create a contact with valid data', async () => {
    // 测试实现
  });

  it('should throw an error for invalid email', async () => {
    // 测试实现
  });
});
```

## 文档贡献

### 类型

- **用户文档**: 面向最终用户的使用指南
- **开发者文档**: API 参考和开发指南
- **贡献文档**: 如何参与项目开发

### 写作指南

- 使用清晰、简洁的语言
- 提供代码示例
- 包含屏幕截图（如果适用）
- 保持文档与代码同步

## Pull Request 流程

### 提交前检查清单

- [ ] 代码遵循项目规范
- [ ] 所有测试通过
- [ ] 添加了必要的测试
- [ ] 更新了相关文档
- [ ] 提交消息遵循规范
- [ ] 没有合并冲突

### 审查流程

1. **自动检查**: CI 会自动运行测试和检查
2. **人工审查**: 维护者会审查代码质量和设计
3. **反馈处理**: 根据反馈修改代码
4. **合并**: 审查通过后合并到主分支

### 审查标准

- **功能性**: 功能是否正确实现
- **性能**: 是否引入性能问题
- **安全性**: 是否存在安全隐患
- **可维护性**: 代码是否易于维护
- **一致性**: 是否与现有代码风格一致

## 发布流程

### 版本控制

我们使用 [语义化版本控制](https://semver.org/)：

- `MAJOR.MINOR.PATCH`
- `1.0.0` - 主要版本，不兼容的 API 变更
- `1.1.0` - 次要版本，向后兼容的功能增加
- `1.1.1` - 补丁版本，向后兼容的问题修复

### 发布步骤

1. **更新 CHANGELOG**: 记录所有变更
2. **更新版本号**: 在 `Cargo.toml` 和 `package.json` 中
3. **创建 Git 标签**: `git tag v1.1.0`
4. **推送标签**: `git push origin v1.1.0`
5. **GitHub Actions**: 自动构建和发布

## 项目结构

```
atomo/
├── .github/              # GitHub 配置
│   ├── workflows/        # CI/CD 工作流
│   └── ISSUE_TEMPLATE/   # Issue 模板
├── crates/               # Rust 库
├── packages/             # 前端包
├── templates/            # 项目模板
├── docs/                 # 文档
└── scripts/              # 构建脚本
```

## 开发工具

### 推荐的 VS Code 扩展

- rust-analyzer
- Prettier
- ESLint
- GitLens
- Thunder Client (API 测试)

### 有用的命令

```bash
# 检查所有代码
cargo check --all

# 运行所有测试
cargo test --all && pnpm test

# 格式化代码
cargo fmt --all && pnpm format

# 生成文档
cargo doc --open

# 运行基准测试
cargo bench
```

## 社区

- **GitHub Discussions**: 技术讨论和问答
- **GitHub Issues**: Bug 报告和功能请求
- **Discord**: 实时聊天（即将开放）

## 获得帮助

如果您在贡献过程中遇到问题：

1. 查看现有的 Issues 和 Discussions
2. 在 GitHub Discussions 中提问
3. 查阅项目文档
4. 联系维护者

## 认可贡献者

我们使用 [All Contributors](https://allcontributors.org/) 来认可所有形式的贡献：

- 💻 代码
- 📖 文档
- 🎨 设计
- 💡 想法
- 🐛 Bug 报告
- 🤔 用户问答

---

感谢您考虑为 Atomo 做出贡献！每一个贡献都让项目变得更好。
