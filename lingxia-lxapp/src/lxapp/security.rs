use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub struct NetworkSecurity {
    /// Domains that are trusted for network requests
    /// If empty, all domains are allowed
    pub trusted_domains: HashSet<String>,
}

impl NetworkSecurity {
    /// Creates a new empty NetworkSecurity configuration
    pub fn new() -> Self {
        Self {
            trusted_domains: HashSet::new(),
        }
    }

    /// Checks if a domain is allowed for network access
    /// If no domains are specified (empty set), all domains are allowed
    pub fn is_domain_allowed(&self, domain: &str) -> bool {
        self.trusted_domains.is_empty() || self.trusted_domains.contains(domain)
    }

    /// Add a trusted domain
    #[allow(dead_code)]
    pub fn add_domain(&mut self, domain: &str) {
        self.trusted_domains.insert(domain.to_string());
    }

    /// Remove a trusted domain
    #[allow(dead_code)]
    pub fn remove_domain(&mut self, domain: &str) {
        self.trusted_domains.remove(domain);
    }

    /// Set trusted domains from a list (replaces current list)
    #[allow(dead_code)]
    pub fn set_domains(&mut self, domains: &[String]) {
        self.trusted_domains.clear();
        for domain in domains {
            self.trusted_domains.insert(domain.clone());
        }
    }

    /// Get current list of trusted domains
    #[allow(dead_code)]
    pub fn get_domains(&self) -> Vec<String> {
        self.trusted_domains.iter().cloned().collect()
    }

    /// Clear all trusted domains (allows all domains)
    #[allow(dead_code)]
    pub fn clear_domains(&mut self) {
        self.trusted_domains.clear();
    }
}
