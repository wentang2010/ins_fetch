use idgenerator::*;

pub struct IdGen;

impl IdGen {
    pub fn new(work_id: u32) -> anyhow::Result<Self> {
        let options = IdGeneratorOptions::new()
            .worker_id(work_id)
            .worker_id_bit_len(6)
            .seq_bit_len(10);
        // Initialize the id generator instance with the option.
        // Other options not set will be given the default value.
        IdInstance::init(options)?;
        Ok(IdGen)
    }

    pub fn next_id() -> i64 {
        IdInstance::next_id()
    }
}
