# 规则引擎底层原理

## 概述

规则引擎是隧道核心的决策中心。它维护一个有序规则列表，对每个连接的元数据进行匹配，找到第一个命中的规则，决定流量走向。

## 规则匹配流程

```
metadata (host, dst_ip, src_ip, port, process, ...)
    │
    v
┌─────────────────────────────────────────────────────┐
│  Rule Chain (按配置顺序)                             │
│                                                      │
│  [1] DOMAIN-SUFFIX,google.com,Proxy     ─ 不匹配 ─>│
│  [2] DOMAIN-KEYWORD,facebook,Proxy      ─ 不匹配 ─>│
│  [3] IP-CIDR,192.168.0.0/16,DIRECT     ─ 不匹配 ─>│
│  [4] GEOIP,CN,DIRECT                   ─ 命中!     │
│                                                      │
│  => 使用 DIRECT 适配器                               │
└─────────────────────────────────────────────────────┘
```

**时间复杂度优化:**
- 朴素实现: O(N) 线性扫描所有规则
- 优化实现: 域名规则用 Trie 树 O(k), IP 规则用前缀树 O(32/128)

## 规则类型详解

### 1. 域名规则

#### DOMAIN (精确匹配)

```rust
pub struct DomainRule {
    domain: String,
    adapter: String,
}

impl Rule for DomainRule {
    fn matches(&self, metadata: &Metadata) -> bool {
        metadata.host.as_deref() == Some(self.domain.as_str())
    }
}
```

#### DOMAIN-SUFFIX (后缀匹配)

```
规则: DOMAIN-SUFFIX,google.com,Proxy
匹配: google.com, www.google.com, mail.google.com
不匹配: notgoogle.com, google.com.cn
```

**高效实现: Domain Trie (域名字典树)**

```rust
use std::collections::HashMap;

/// 域名 Trie 节点
struct TrieNode {
    children: HashMap<String, TrieNode>,
    /// 如果此节点是某条规则的终点, 记录适配器名称
    adapter: Option<String>,
    /// 是否为通配符节点
    is_wildcard: bool,
}

/// 域名 Trie 树
/// 域名按 "." 分割后, 从右到左插入
/// 例如 "www.google.com" 插入顺序: com -> google -> www
pub struct DomainTrie {
    root: TrieNode,
}

impl DomainTrie {
    /// 插入域名后缀规则
    pub fn insert_suffix(&mut self, domain: &str, adapter: String) {
        let labels: Vec<&str> = domain.split('.').rev().collect();
        let mut node = &mut self.root;
        for label in labels {
            node = node.children
                .entry(label.to_string())
                .or_insert_with(TrieNode::new);
        }
        node.adapter = Some(adapter);
    }

    /// 查找匹配的规则
    /// 返回最长匹配的适配器名称
    pub fn lookup(&self, domain: &str) -> Option<&str> {
        let labels: Vec<&str> = domain.split('.').rev().collect();
        let mut node = &self.root;
        let mut last_match = None;

        for label in labels {
            // 检查通配符
            if let Some(wildcard) = node.children.get("*") {
                if let Some(ref adapter) = wildcard.adapter {
                    last_match = Some(adapter.as_str());
                }
            }

            match node.children.get(label) {
                Some(child) => {
                    node = child;
                    if let Some(ref adapter) = node.adapter {
                        last_match = Some(adapter.as_str());
                    }
                }
                None => break,
            }
        }

        last_match
    }
}

// 使用示例:
// trie.insert_suffix("google.com", "Proxy");
// trie.lookup("www.google.com")  => Some("Proxy")
// trie.lookup("google.com")      => Some("Proxy")
// trie.lookup("notgoogle.com")   => None
```

#### DOMAIN-KEYWORD (关键词匹配)

```rust
pub struct DomainKeywordRule {
    keyword: String,
    adapter: String,
}

impl Rule for DomainKeywordRule {
    fn matches(&self, metadata: &Metadata) -> bool {
        metadata.host.as_ref()
            .map(|h| h.contains(&self.keyword))
            .unwrap_or(false)
    }
}
```

#### DOMAIN-REGEX (正则匹配)

```rust
use regex::Regex;

pub struct DomainRegexRule {
    pattern: Regex,
    adapter: String,
}

impl Rule for DomainRegexRule {
    fn matches(&self, metadata: &Metadata) -> bool {
        metadata.host.as_ref()
            .map(|h| self.pattern.is_match(h))
            .unwrap_or(false)
    }
}
```

### 2. IP 规则

#### IP-CIDR (CIDR 匹配)

```
规则: IP-CIDR,192.168.0.0/16,DIRECT
匹配: 192.168.0.0 ~ 192.168.255.255 范围内的所有 IP
```

**高效实现: 压缩前缀树 (Patricia Trie / Radix Tree)**

```rust
use std::net::IpAddr;

/// CIDR 前缀树节点
enum CidrNode {
    Leaf {
        adapter: String,
    },
    Branch {
        left: Option<Box<CidrNode>>,   // bit = 0
        right: Option<Box<CidrNode>>,  // bit = 1
    },
}

/// IPv4 CIDR 匹配树
pub struct CidrTrie {
    root: CidrNode,
}

impl CidrTrie {
    /// 插入 CIDR 规则
    pub fn insert(&mut self, cidr: &str, adapter: String) {
        let (ip, prefix_len) = parse_cidr(cidr);
        let bits = ip_to_bits(ip);

        let mut node = &mut self.root;
        for i in 0..prefix_len {
            let bit = (bits >> (31 - i)) & 1;
            node = match bit {
                0 => node.ensure_left(),
                1 => node.ensure_right(),
                _ => unreachable!(),
            };
        }
        *node = CidrNode::Leaf { adapter };
    }

    /// 最长前缀匹配
    pub fn lookup(&self, ip: IpAddr) -> Option<&str> {
        let bits = ip_to_bits(ip);
        let mut node = &self.root;
        let mut last_match = None;

        for i in 0..32 {
            if let CidrNode::Leaf { ref adapter } = node {
                last_match = Some(adapter.as_str());
            }
            let bit = (bits >> (31 - i)) & 1;
            match (bit, node) {
                (0, CidrNode::Branch { left: Some(ref l), .. }) => node = l,
                (1, CidrNode::Branch { right: Some(ref r), .. }) => node = r,
                _ => break,
            }
        }

        last_match
    }
}
```

实际项目中推荐使用 `ipnet` crate 和 `ip_network_table` crate。

### 3. GeoIP 规则

使用 MaxMind MMDB 数据库查找 IP 对应的国家/地区:

```rust
use maxminddb::Reader;
use std::net::IpAddr;

pub struct GeoIpRule {
    country_code: String,  // "CN", "US", etc.
    adapter: String,
    mmdb: Arc<Reader<Vec<u8>>>,
}

impl Rule for GeoIpRule {
    fn matches(&self, metadata: &Metadata) -> bool {
        let ip = match metadata.dst_ip {
            Some(ip) => ip,
            None => return false,
        };

        match self.mmdb.lookup::<maxminddb::geoip2::Country>(ip) {
            Ok(record) => {
                record.country
                    .and_then(|c| c.iso_code)
                    .map(|code| code == self.country_code)
                    .unwrap_or(false)
            }
            Err(_) => false,
        }
    }

    fn should_resolve_ip(&self) -> bool {
        true  // GeoIP 需要 IP 才能匹配
    }
}
```

### 4. GeoSite 规则

GeoSite 是域名分类数据库 (来自 v2ray/domain-list-community):

```rust
/// GeoSite 数据格式 (protobuf)
/// 按类别 (category) 组织域名列表
pub struct GeoSiteRule {
    category: String,  // "cn", "google", "facebook", etc.
    adapter: String,
    /// 内部使用 DomainTrie 加速匹配
    trie: DomainTrie,
}

impl GeoSiteRule {
    pub fn from_dat(dat_path: &str, category: &str, adapter: String) -> Result<Self, Error> {
        // 1. 解析 protobuf 格式的 geosite.dat
        let data = std::fs::read(dat_path)?;
        let site_list = parse_geosite_dat(&data)?;

        // 2. 找到指定类别
        let domains = site_list.get(category)
            .ok_or(Error::Config(format!("GeoSite category not found: {}", category)))?;

        // 3. 构建 Trie 树
        let mut trie = DomainTrie::new();
        for domain in domains {
            match domain.domain_type {
                DomainType::Full => trie.insert_exact(&domain.value, adapter.clone()),
                DomainType::Domain => trie.insert_suffix(&domain.value, adapter.clone()),
                DomainType::Keyword => { /* 关键词存储在单独列表 */ }
                DomainType::Regex => { /* 正则存储在单独列表 */ }
            }
        }

        Ok(Self { category: category.to_string(), adapter, trie })
    }
}
```

### 5. 进程规则

根据发起连接的进程名匹配:

```rust
pub struct ProcessRule {
    process_name: String,
    adapter: String,
}

impl Rule for ProcessRule {
    fn matches(&self, metadata: &Metadata) -> bool {
        metadata.process_name.as_deref() == Some(self.process_name.as_str())
    }

    fn should_find_process(&self) -> bool {
        true  // 需要查找进程信息
    }
}
```

**进程查找的平台实现:**

```rust
/// 根据 (源 IP, 源端口) 查找进程
fn find_process(src_addr: &SocketAddr) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        // 解析 /proc/net/tcp 或 /proc/net/tcp6
        // 找到匹配 (src_ip, src_port) 的 inode
        // 遍历 /proc/*/fd/ 找到持有该 inode 的进程
        find_process_linux(src_addr)
    }

    #[cfg(target_os = "macos")]
    {
        // 使用 libproc::proc_pidinfo 或 lsof
        find_process_macos(src_addr)
    }

    #[cfg(target_os = "windows")]
    {
        // 使用 GetExtendedTcpTable / GetExtendedUdpTable
        find_process_windows(src_addr)
    }
}
```

### 6. 逻辑组合规则

```rust
/// AND 规则: 所有子规则都匹配才算匹配
pub struct AndRule {
    rules: Vec<Box<dyn Rule>>,
    adapter: String,
}

impl Rule for AndRule {
    fn matches(&self, metadata: &Metadata) -> bool {
        self.rules.iter().all(|r| r.matches(metadata))
    }

    fn should_resolve_ip(&self) -> bool {
        self.rules.iter().any(|r| r.should_resolve_ip())
    }
}

/// OR 规则: 任一子规则匹配即算匹配
pub struct OrRule {
    rules: Vec<Box<dyn Rule>>,
    adapter: String,
}

/// NOT 规则: 子规则不匹配才算匹配
pub struct NotRule {
    rule: Box<dyn Rule>,
    adapter: String,
}
```

### 7. RuleSet (远程规则集)

```rust
/// 规则集: 从远程 URL 加载大量规则
pub struct RuleSetRule {
    provider_name: String,
    adapter: String,
    /// 运行时通过 RuleProvider 获取实际规则
    provider: Arc<dyn RuleProvider>,
}

impl Rule for RuleSetRule {
    fn matches(&self, metadata: &Metadata) -> bool {
        self.provider.matches(metadata)
    }
}
```

## 规则解析工厂

```rust
/// 从配置字符串解析规则
pub fn parse_rule(line: &str) -> Result<Box<dyn Rule>, Error> {
    let parts: Vec<&str> = line.splitn(3, ',').collect();

    match parts[0] {
        "DOMAIN" => Ok(Box::new(DomainRule {
            domain: parts[1].to_string(),
            adapter: parts[2].to_string(),
        })),
        "DOMAIN-SUFFIX" => Ok(Box::new(DomainSuffixRule {
            suffix: parts[1].to_string(),
            adapter: parts[2].to_string(),
        })),
        "DOMAIN-KEYWORD" => Ok(Box::new(DomainKeywordRule {
            keyword: parts[1].to_string(),
            adapter: parts[2].to_string(),
        })),
        "IP-CIDR" | "IP-CIDR6" => Ok(Box::new(IpCidrRule::new(parts[1], parts[2])?)),
        "GEOIP" => Ok(Box::new(GeoIpRule::new(parts[1], parts[2])?)),
        "GEOSITE" => Ok(Box::new(GeoSiteRule::new(parts[1], parts[2])?)),
        "PROCESS-NAME" => Ok(Box::new(ProcessRule {
            process_name: parts[1].to_string(),
            adapter: parts[2].to_string(),
        })),
        "SRC-PORT" => Ok(Box::new(SrcPortRule::new(parts[1], parts[2])?)),
        "DST-PORT" => Ok(Box::new(DstPortRule::new(parts[1], parts[2])?)),
        "MATCH" => Ok(Box::new(MatchRule {
            adapter: parts[1].to_string(),
        })),
        _ => Err(Error::Config(format!("Unknown rule type: {}", parts[0]))),
    }
}
```

## 性能优化: 混合匹配引擎

将所有规则合并到几个高效数据结构中，避免逐条线性扫描:

```rust
/// 优化后的规则引擎
pub struct OptimizedRuleEngine {
    /// 域名规则: 合并到单个 Trie 树
    domain_trie: DomainTrie,
    /// IP 规则: 合并到 CIDR 前缀树
    cidr_trie: CidrTrie,
    /// 其他规则: 保持有序列表 (数量通常很少)
    ordered_rules: Vec<Box<dyn Rule>>,
    /// 默认规则
    default_adapter: String,
}

impl OptimizedRuleEngine {
    pub fn matches(&self, metadata: &Metadata) -> &str {
        // 1. 先查域名 Trie (O(k), k = 域名标签数)
        if let Some(host) = &metadata.host {
            if let Some(adapter) = self.domain_trie.lookup(host) {
                return adapter;
            }
        }

        // 2. 查 IP CIDR 树 (O(32))
        if let Some(ip) = metadata.dst_ip {
            if let Some(adapter) = self.cidr_trie.lookup(ip) {
                return adapter;
            }
        }

        // 3. 回退到有序列表 (通常只有少量 PROCESS/PORT 等规则)
        for rule in &self.ordered_rules {
            if rule.matches(metadata) {
                return rule.adapter();
            }
        }

        &self.default_adapter
    }
}
```

> **注意**: 优化版引擎改变了规则优先级语义。实际 Clash 使用严格的声明顺序优先级。如果用户需要域名规则优先于 IP 规则（或反之），优化版需要额外处理。通常保持线性扫描但对域名查找和 IP 查找使用索引加速是更安全的做法。
