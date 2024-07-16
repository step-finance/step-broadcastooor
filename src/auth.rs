use serde_derive::Deserialize;

#[derive(Debug, Deserialize)]
pub struct AuthData {
    pub token: String,
}

pub mod claims {

    use std::collections::HashMap;

    use serde_derive::Deserialize;
    use serde_derive::Serialize;

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Root {
        pub cluster: String,
        pub api_key: Option<String>,
        pub public_key: Option<String>,
        pub roles_and_products_map: HashMap<String, RolesAndProducts>,
        pub iat: i64,
        pub exp: i64,
    }
    impl Root {
        
        #[inline]
        pub fn has_role(&self, role: &String, regarding: Option<&str>) -> bool {
            self.global_roles_and_products().roles.contains(role)
                || regarding
                    .map(|a| self.roles_regarding(a).roles.contains(role))
                    .unwrap_or(false)
        }

        #[inline]
        pub fn global_roles_and_products(&self) -> &RolesAndProducts {
            self.roles_regarding("globalRolesAndProducts")
        }

        #[inline]
        pub fn roles_regarding(&self, regarding: &str) -> &RolesAndProducts {
            self.roles_and_products_map.get(regarding).unwrap()
        }
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RolesAndProducts {
        pub roles: Vec<String>,
        pub products: Vec<Product>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Product {
        pub family: String,
        pub expires_ts: i64,
        pub int_udfs: Vec<i64>,
    }
}
