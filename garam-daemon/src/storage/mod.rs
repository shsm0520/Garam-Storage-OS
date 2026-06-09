pub trait StorageBackend {
    fn create_pool(&self, name: &str, raid_type: &str, disks: &[&str]) -> Result<String, String>;
    fn list_pools(&self) -> Result<String, String>;
}

// 🔗 이 하위 폴더에 zfs.rs, ghs.rs, lvm.rs 파일이 상주하고 있음을 선포!
pub mod zfs;
pub mod ghs;
pub mod lvm;