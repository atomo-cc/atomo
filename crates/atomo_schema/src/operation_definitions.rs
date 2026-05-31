use crate::types::FieldType;
use anyhow::Result;

/// 统一的操作符定义 - Schema和Runtime的单一数据源
/// 确保GraphQL Schema生成和Resolver生成使用完全相同的操作符定义
#[derive(Debug, Clone)]
pub struct OperationDefinitions {
    pub comparison_ops: Vec<ComparisonOp>,
    pub logical_ops: Vec<LogicalOp>,
}

/// 比较操作符定义
#[derive(Debug, Clone)]
pub struct ComparisonOp {
    pub name: String,               // GraphQL中的名称: "_eq", "_neq", "_gt"
    pub sql_operator: String,       // SQL中的操作符: "=", "!=", ">"
    pub applies_to: Vec<FieldType>, // 适用的字段类型
    pub description: String,        // 操作符描述
}

/// 逻辑操作符定义
#[derive(Debug, Clone)]
pub struct LogicalOp {
    pub name: String,        // "_and", "_or", "_not"
    pub sql_keyword: String, // "AND", "OR", "NOT"
    pub description: String,
}

impl OperationDefinitions {
    /// 获取标准的Hasura v2操作符定义
    pub fn hasura_v2_standard() -> Self {
        Self {
            comparison_ops: vec![
                ComparisonOp {
                    name: "_eq".to_string(),
                    sql_operator: "=".to_string(),
                    applies_to: vec![
                        FieldType::String,
                        FieldType::Number,
                        FieldType::Boolean,
                        FieldType::Date,
                        FieldType::DateTime,
                        FieldType::EntityId,
                    ],
                    description: "Equal to".to_string(),
                },
                ComparisonOp {
                    name: "_neq".to_string(),
                    sql_operator: "!=".to_string(),
                    applies_to: vec![
                        FieldType::String,
                        FieldType::Number,
                        FieldType::Boolean,
                        FieldType::Date,
                        FieldType::DateTime,
                        FieldType::EntityId,
                    ],
                    description: "Not equal to".to_string(),
                },
                ComparisonOp {
                    name: "_gt".to_string(),
                    sql_operator: ">".to_string(),
                    applies_to: vec![
                        FieldType::String,
                        FieldType::Number,
                        FieldType::Date,
                        FieldType::DateTime,
                    ],
                    description: "Greater than".to_string(),
                },
                ComparisonOp {
                    name: "_gte".to_string(),
                    sql_operator: ">=".to_string(),
                    applies_to: vec![
                        FieldType::String,
                        FieldType::Number,
                        FieldType::Date,
                        FieldType::DateTime,
                    ],
                    description: "Greater than or equal to".to_string(),
                },
                ComparisonOp {
                    name: "_lt".to_string(),
                    sql_operator: "<".to_string(),
                    applies_to: vec![
                        FieldType::String,
                        FieldType::Number,
                        FieldType::Date,
                        FieldType::DateTime,
                    ],
                    description: "Less than".to_string(),
                },
                ComparisonOp {
                    name: "_lte".to_string(),
                    sql_operator: "<=".to_string(),
                    applies_to: vec![
                        FieldType::String,
                        FieldType::Number,
                        FieldType::Date,
                        FieldType::DateTime,
                    ],
                    description: "Less than or equal to".to_string(),
                },
                ComparisonOp {
                    name: "_like".to_string(),
                    sql_operator: "LIKE".to_string(),
                    applies_to: vec![FieldType::String],
                    description: "SQL LIKE pattern matching".to_string(),
                },
                ComparisonOp {
                    name: "_ilike".to_string(),
                    sql_operator: "ILIKE".to_string(),
                    applies_to: vec![FieldType::String],
                    description: "Case-insensitive SQL LIKE pattern matching".to_string(),
                },
                ComparisonOp {
                    name: "_in".to_string(),
                    sql_operator: "IN".to_string(),
                    applies_to: vec![FieldType::String, FieldType::Number, FieldType::EntityId],
                    description: "In array".to_string(),
                },
                ComparisonOp {
                    name: "_nin".to_string(),
                    sql_operator: "NOT IN".to_string(),
                    applies_to: vec![FieldType::String, FieldType::Number, FieldType::EntityId],
                    description: "Not in array".to_string(),
                },
                ComparisonOp {
                    name: "_is_null".to_string(),
                    sql_operator: "IS NULL".to_string(),
                    applies_to: vec![
                        FieldType::String,
                        FieldType::Number,
                        FieldType::Boolean,
                        FieldType::Date,
                        FieldType::DateTime,
                        FieldType::EntityId,
                    ],
                    description: "Is null or not null".to_string(),
                },
            ],
            logical_ops: vec![
                LogicalOp {
                    name: "_and".to_string(),
                    sql_keyword: "AND".to_string(),
                    description: "Logical AND".to_string(),
                },
                LogicalOp {
                    name: "_or".to_string(),
                    sql_keyword: "OR".to_string(),
                    description: "Logical OR".to_string(),
                },
                LogicalOp {
                    name: "_not".to_string(),
                    sql_keyword: "NOT".to_string(),
                    description: "Logical NOT".to_string(),
                },
            ],
        }
    }

    /// 获取适用于特定字段类型的比较操作符
    pub fn get_comparison_ops_for_type(&self, field_type: &FieldType) -> Vec<&ComparisonOp> {
        self.comparison_ops
            .iter()
            .filter(|op| op.applies_to.contains(field_type))
            .collect()
    }

    /// 根据名称获取比较操作符
    pub fn get_comparison_op_by_name(&self, name: &str) -> Option<&ComparisonOp> {
        self.comparison_ops.iter().find(|op| op.name == name)
    }

    /// 验证操作符定义的一致性
    pub fn validate(&self) -> Result<()> {
        // 检查是否有重复的操作符名称
        let mut names = std::collections::HashSet::new();
        for op in &self.comparison_ops {
            if !names.insert(&op.name) {
                anyhow::bail!("Duplicate comparison operator name: {}", op.name);
            }
        }

        for op in &self.logical_ops {
            if !names.insert(&op.name) {
                anyhow::bail!("Duplicate logical operator name: {}", op.name);
            }
        }

        // 验证必要的操作符存在
        let required_ops = ["_eq", "_neq", "_and", "_or", "_not"];
        for required in &required_ops {
            if !self.comparison_ops.iter().any(|op| op.name == *required)
                && !self.logical_ops.iter().any(|op| op.name == *required)
            {
                anyhow::bail!("Required operator missing: {}", required);
            }
        }

        Ok(())
    }

    /// 生成操作符的详细报告（用于调试和文档）
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# Hasura v2 Operation Definitions\n\n");

        report.push_str("## Comparison Operators\n");
        for op in &self.comparison_ops {
            report.push_str(&format!(
                "- **{}**: {} (SQL: `{}`)\n  - Applies to: {:?}\n",
                op.name, op.description, op.sql_operator, op.applies_to
            ));
        }

        report.push_str("\n## Logical Operators\n");
        for op in &self.logical_ops {
            report.push_str(&format!(
                "- **{}**: {} (SQL: `{}`)\n",
                op.name, op.description, op.sql_keyword
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hasura_v2_standard_validation() {
        let ops = OperationDefinitions::hasura_v2_standard();
        assert!(ops.validate().is_ok());
    }

    #[test]
    fn test_get_comparison_ops_for_string() {
        let ops = OperationDefinitions::hasura_v2_standard();
        let string_ops = ops.get_comparison_ops_for_type(&FieldType::String);

        // String应该支持大部分操作符
        assert!(string_ops.len() >= 8);
        assert!(string_ops.iter().any(|op| op.name == "_eq"));
        assert!(string_ops.iter().any(|op| op.name == "_like"));
    }

    #[test]
    fn test_get_comparison_op_by_name() {
        let ops = OperationDefinitions::hasura_v2_standard();

        let eq_op = ops.get_comparison_op_by_name("_eq").unwrap();
        assert_eq!(eq_op.sql_operator, "=");

        let neq_op = ops.get_comparison_op_by_name("_neq").unwrap();
        assert_eq!(neq_op.sql_operator, "!=");

        assert!(ops.get_comparison_op_by_name("_invalid").is_none());
    }
}
