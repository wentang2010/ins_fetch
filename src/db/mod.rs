use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Result};
use bincode::error::AllowedEnumVariants;
use futures::stream::TryStreamExt;

use format_sql_query::QuotedData;
use futures_util::{lock::Mutex, Future};
use once_cell::sync::OnceCell;
use serde_json::json;
use solana_sdk::pubkey::Pubkey;
use tracing::{debug, error, info, warn};

use crate::{
    common::{
        dao::{
            self, InscriptionList, OneAccountInfo, OneGroupReplicaIns, ReplicaInsGroupList,
            ReplicaInsList, SocialInfoDB, StoreData,
        },
        idgen::IdGen,
        structs::{
            ChainEvent, CopyInsData, EventName, InsData, Inscription, InscriptionData, ReplicaIns,
            ReplicaTx,
        },
        util,
    },
    global_var, Err,
};

use mongodb::{
    bson::{self, doc, Bson, Document},
    options::{ClientOptions, CountOptions, FindOptions, UpdateModifications, UpdateOptions},
    Client as MongoClient, Collection, Cursor, Database, IndexModel,
};

// pub async fn save_poolmeta_info(
//     db_pool: &Pool,
//     chain_id: String,
//     pool_address: Address,
// ) -> Result<()> {
//     Ok(())
// }

static DB_INS: OnceCell<DB> = OnceCell::new();

pub async fn create_db_ins() -> Result<DB> {
    let application = global_var::get_app_conf();

    let client_opts = ClientOptions::parse(application.db.url.as_str()).await?;

    // Get a handle to the deployment.
    let client = MongoClient::with_options(client_opts)?;
    let db = client.database(application.db.db_name.as_str());

    Ok(DB::new(db).await?)

    // .collection::<Book>("test");

    // let find_options = FindOptions::builder()..build();
    // let mut cursor = collection.find(doc! {"x":1}, None).await?;
    // // Iterate over the results of the cursor.
    // while let Some(book) = cursor.try_next().await? {
    //     println!("title: {}", book.x);
    // }
}

pub fn get_db_ins() -> &'static DB {
    return DB_INS.get().unwrap();
}

pub fn set_db_ins(db: DB) {
    DB_INS.set(db).unwrap();
}

async fn get_cursor_count(count_search: Cursor<Document>) -> Result<u64> {
    let mut count_search = count_search;
    let mut query_count = 0;
    while let Some(one_row) = count_search
        .try_next()
        .await
        .map_err(|e| anyhow!("try query count next failed: {:?}", e))?
    {
        query_count = one_row.get_i32("count").unwrap_or_default() as u64;
        break;
    }

    Ok(query_count)
}

#[derive(Debug)]
pub struct DB {
    pub db: Database, // pub collection,
                      // pub conf: DBConfig
}

impl DB {
    pub async fn new(db: Database) -> Result<DB> {
        let self_db = DB { db };
        let ins_coll = self_db.get_ins_coll()?;
        ins_coll
            .create_index(
                IndexModel::builder().keys(doc! {"key":"hashed"}).build(),
                None,
            )
            .await?;

        let replica_ins_coll = self_db.get_replica_ins_coll()?;
        replica_ins_coll
            .create_index(
                IndexModel::builder().keys(doc! {"key":"hashed"}).build(),
                None,
            )
            .await?;

        let replica_ins_tx_coll = self_db.get_replica_tx_coll()?;
        replica_ins_tx_coll
            .create_index(
                IndexModel::builder().keys(doc! {"key":"hashed"}).build(),
                None,
            )
            .await?;

        let social_info_coll = self_db.get_social_info_coll()?;
        social_info_coll
            .create_index(
                IndexModel::builder().keys(doc! {"key":"hashed"}).build(),
                None,
            )
            .await?;

        let storage_coll = self_db.get_storage_coll()?;
        storage_coll
            .create_index(
                IndexModel::builder().keys(doc! {"key":"hashed"}).build(),
                None,
            )
            .await?;

        Ok(self_db)
    }

    pub fn get_ins_coll(&self) -> Result<Collection<Inscription>> {
        let coll = self.db.collection("inscription");
        Ok(coll)
    }

    pub fn get_replica_ins_coll(&self) -> Result<Collection<ReplicaIns>> {
        let coll = self.db.collection("replica_ins");
        Ok(coll)
    }

    pub fn get_replica_tx_coll(&self) -> Result<Collection<ReplicaTx>> {
        let coll = self.db.collection("replica_tx");
        Ok(coll)
    }

    pub fn get_social_info_coll(&self) -> Result<Collection<SocialInfoDB>> {
        let coll = self.db.collection("social_info");
        Ok(coll)
    }

    pub fn get_storage_coll(&self) -> Result<Collection<StoreData>> {
        let coll = self.db.collection("store_data");
        Ok(coll)
    }

    // pub fn get_conf_collection(&self) -> Result<Collection<Conf>> {
    //     let coll = self.db.collection("conf");
    //     Ok(coll)
    // }

    // pub async fn get_scan_start_point(&self) -> Result<u64> {
    //     // let db_ins = get_db_ins();
    //     Ok(util::get_file_start()?)
    // }

    // pub async fn save_scan_start_point(&self, point: u64) -> Result<()> {
    //     util::save_start(point);
    //     Ok(())
    // }

    pub async fn is_ins_exist(&self, key: Pubkey) -> Result<bool> {
        let collection = self.get_ins_coll()?;
        let count = collection
            .count_documents(
                doc! {"key": key.to_string()},
                CountOptions::builder().limit(1).build(),
            )
            .await?;

        Ok(count > 0)
    }

    pub async fn is_ins_can_be_updated(&self, key: Pubkey) -> Result<bool> {
        let collection = self.get_ins_coll()?;
        let count = collection
            .count_documents(
                doc! {"key": key.to_string(), "data.deployed": false},
                CountOptions::builder().limit(1).build(),
            )
            .await?;

        Ok(count > 0)
    }

    pub async fn get_inscription_count(&self, count_type: u8) -> Result<u64> {
        let collection = self.get_ins_coll()?;

        if count_type == 1 {
            // only deployed
            let count = collection
                .count_documents(doc! {"data.deployed":true}, None)
                .await?;
            return Ok(count);
        }

        // all counts
        let count = collection.count_documents(None, None).await?;
        return Ok(count);
    }

    pub async fn get_replica_ins_count(&self) -> Result<u64> {
        let collection = self.get_replica_ins_coll()?;
        let count = collection.count_documents(None, None).await?;

        Ok(count)
    }

    pub async fn is_replica_ins_exist(&self, key: Pubkey) -> Result<bool> {
        let collection = self.get_replica_ins_coll()?;
        let count = collection
            .count_documents(
                doc! {"key": key.to_string()},
                CountOptions::builder().limit(1).build(),
            )
            .await?;

        Ok(count > 0)
    }

    pub async fn is_replica_tx_exist(&self, key: &str) -> Result<bool> {
        let collection = self.get_replica_tx_coll()?;
        let count = collection
            .count_documents(
                doc! {"key": key.to_string()},
                CountOptions::builder().limit(1).build(),
            )
            .await?;

        Ok(count > 0)
    }

    async fn insert_into_replica_tx(
        &self,
        evt: &ChainEvent,
        replica_ins: &ReplicaIns,
    ) -> Result<()> {
        let collection = self.get_replica_tx_coll()?;
        // tx key is the ins key.
        let exist = self.is_replica_tx_exist(evt.tx.as_str()).await?;
        if exist {
            return Ok(());
        }

        let one_tx = ReplicaTx {
            key: evt.tx.to_string(),
            evt: evt.clone(),
            replica_ins: replica_ins.clone(),
            sort_value: 0,
        };

        collection.insert_one(one_tx, None).await?;

        Ok(())
    }

    pub async fn insert_if_not_exist_evts(&self, chain_evts: Vec<ChainEvent>) -> Result<()> {
        for one_evt in chain_evts.iter() {
            if EventName::InscriptionCreated == one_evt.evt.name {
                let collection = self.get_ins_coll()?;
                let ins_data = InsData::new(&one_evt.evt.data_raw);

                let exist = self
                    .is_ins_exist(ins_data.key)
                    .await
                    .map_err(|e| anyhow!("error in is_ins_exist, {:?}", e))?;
                if exist {
                    debug!("ins exist not insert");
                    continue;
                }

                let one_ins_rs = Inscription::from_data(&ins_data)
                    .await
                    .map_err(|e| anyhow!("error in Inscription::from_data, {:?}", e));
                let one_ins;
                if one_ins_rs.is_err() {
                    error!(
                        "data structure is not compatible for Inscription, data: {:?}",
                        &ins_data
                    );
                    continue;
                } else {
                    one_ins = one_ins_rs.unwrap();
                }

                collection
                    .insert_one(one_ins, None)
                    .await
                    .map_err(|e| anyhow!("error in Inscription insert_one, {:?}", e))?;
                println!("insert one ins");
                // collection.find_one_and_update(filter, update, options)
            } else if EventName::ReplicaInsCreated == one_evt.evt.name {
                let collection = self.get_replica_ins_coll()?;
                let rep_ins_data = CopyInsData::new(&one_evt.evt.data_raw);

                debug!("rep ins data {:?}", rep_ins_data);

                let exist = self
                    .is_replica_ins_exist(rep_ins_data.replica_ins)
                    .await
                    .map_err(|e| anyhow!("error in is_replica_ins_exist, {:?}", e))?;

                if exist {
                    debug!("replica exist not insert");
                    continue;
                }

                let one_replica_ins_rs = ReplicaIns::from_data(&rep_ins_data)
                    .await
                    .map_err(|e| anyhow!("error in ReplicaIns::from_data, {:?}", e));

                let one_replica_ins;
                if one_replica_ins_rs.is_err() {
                    error!(
                        "data structure is not compatible for ReplicaIns, data: {:?}",
                        &rep_ins_data
                    );
                    continue;
                } else {
                    one_replica_ins = one_replica_ins_rs.unwrap();
                }

                // debug!("one_replica_ins {:?}", one_replica_ins);
                self.insert_into_replica_tx(one_evt, &one_replica_ins)
                    .await
                    .map_err(|e| anyhow!("error in insert_into_replica_tx, {:?}", e))?;
                println!("insert one replica tx");

                collection
                    .insert_one(one_replica_ins, None)
                    .await
                    .map_err(|e| anyhow!("error in ReplicaIns insert_one, {:?}", e))?;
                println!("insert one replica");

                // self.update_all_ins_used_num(true)
                //     .await
                //     .map_err(|e| anyhow!("error in update_all_ins_used_num, {:?}", e))?;
                // println!("update all used num");
                // collection.find_one_and_update(filter, update, options)
            }
        }

        Ok(())
    }

    pub async fn insert_if_not_exist_inscription(
        &self,
        inscription_list: Vec<OneAccountInfo>,
    ) -> Result<()> {
        for one_ac in inscription_list.iter() {
            let collection = self.get_ins_coll()?;
            // let ins_data = InsData::new(&one_evt.evt.data_raw);

            let mut can_be_deleted = false;

            // let exist = self
            //     .is_ins_exist(one_ac.pubkey)
            //     .await
            //     .map_err(|e| anyhow!("error in is_ins_exist, {:?}", e))?;

            // if exist {
            //     // if self.is_ins_can_be_updated(one_ac.pubkey).await? {
            //     //     can_be_deleted = true;
            //     // } else {
            //     //     debug!("ins exist not insert");
            //     //     continue;
            //     // }
            // }

            let one_ins_rs = Inscription::from_account(one_ac)
                .await
                .map_err(|e| anyhow!("error in Inscription::from_account, {:?}", e));
            let one_ins;
            if one_ins_rs.is_err() {
                error!(
                    "data structure is not compatible for Inscription, pk: {:?}, len {}, err: {:?}",
                    &one_ac.pubkey,
                    one_ac.data.len(),
                    one_ins_rs.unwrap_err()
                );
                continue;
            } else {
                one_ins = one_ins_rs.unwrap();
            }

            if one_ins.data.created_ts == 0 {
                // this is a empty account we skip it.
                continue;
            }

            if one_ins.data.index == 0 {
                // this should not happen. we will ignore it.
                error!(
                    "index is zero in inscription. we ignore it. inscripiton: {:?}, data: {:?}",
                    one_ins.key, one_ins.data
                );
                continue;
            }

            // we need always delete it. as we need to update it.
            collection
                .delete_one(doc! {"key": one_ac.pubkey.to_string()}, None)
                .await
                .map_err(|e| anyhow!("delete one ins error {:?}", e))?;

            // let update_options = UpdateOptions::builder().upsert(true).build();
            // collection.update_one(doc!{"key":one_ac.pubkey.to_string()}, UpdateModifications::Document(one_ins), update_options).await?;

            collection
                .insert_one(one_ins, None)
                .await
                .map_err(|e| anyhow!("error in Inscription insert_one, {:?}", e))?;
            println!("insert one ins");
        }

        Ok(())
    }

    pub async fn insert_if_not_exist_replica_ins(
        &self,
        replica_list: Vec<OneAccountInfo>,
    ) -> Result<()> {
        let collection = self.get_replica_ins_coll()?;
        for one_ac in replica_list.iter() {
            let exist = self
                .is_replica_ins_exist(one_ac.pubkey)
                .await
                .map_err(|e| anyhow!("error in is_replica_ins_exist, {:?}", e))?;

            if exist {
                debug!("replica exist not insert");
                continue;
            }

            let one_replica_ins_rs = ReplicaIns::from_account(one_ac)
                .await
                .map_err(|e| anyhow!("error in ReplicaIns::from_data, {:?}", e));

            let one_replica_ins;
            if one_replica_ins_rs.is_err() {
                error!(
                    "data structure is not compatible for ReplicaIns, data: {:?}, len: {}",
                    &one_ac.data,
                    one_ac.data.len()
                );
                continue;
            } else {
                one_replica_ins = one_replica_ins_rs.unwrap();
            }

            // debug!("one_replica_ins {:?}", one_replica_ins);
            // self.insert_into_replica_tx(one_evt, &one_replica_ins)
            //     .await
            //     .map_err(|e| anyhow!("error in insert_into_replica_tx, {:?}", e))?;
            // println!("insert one replica tx");

            collection
                .insert_one(one_replica_ins, None)
                .await
                .map_err(|e| anyhow!("error in ReplicaIns insert_one, {:?}", e))?;
            println!("insert one replica");

            // self.update_all_ins_used_num(true)
            //     .await
            //     .map_err(|e| anyhow!("error in update_all_ins_used_num, {:?}", e))?;
            // println!("update all used num");
        }

        Ok(())
    }

    pub async fn save_or_get_data(&self, data: StoreData, del: bool) -> Result<Option<StoreData>> {
        let key = data.key.trim();
        if key.len() == 0 {
            return Err(anyhow!("key is empty"));
        }

        let collection = self.get_storage_coll()?;

        // query only
        if data.value.is_none() && !del {
            let doc = collection
                .find_one(doc! {"key":data.key.to_string()}, None)
                .await?;
            return Ok(doc);
        }

        collection
            .delete_one(doc! {"key": data.key.to_string()}, None)
            .await
            .map_err(|e| anyhow!("delete one storeData error {:?}", e))?;

        if del {
            println!("delete one storeData");
            return Ok(None);
        }

        collection
            .insert_one(data.clone(), None)
            .await
            .map_err(|e| anyhow!("error in save_or_get_data, {:?}", e))?;
        println!("insert one storeData");
        return Ok(Some(data));
    }
    /*
    pub async fn update_all_ins_used_num(&self, refresh: bool) -> Result<()> {
        let collection = self.get_ins_coll()?;
        //NOTE: IMPORTANT before invoke get_account_book_list the return value will be released before invoke this.
        let ab = util::get_account_book_sync().await?;
        debug!("ab {:?}", ab);
        debug!("ab len {:?}", ab.list.len());

        let mut cursor = collection.find(None, None).await?;

        let mut update_arr = vec![];
        while let Some(mut ins) = cursor
            .try_next()
            .await
            .map_err(|e| anyhow!("try next failed: {:?}", e))?
        {
            if ins.data.index == 0 {
                // this should not be happen.
                error!("inscription index should not be zero");
                continue;
            }

            let index = ins.data.index as usize - 1;
            if index >= ab.list.len() {
                error!("inscription index out of bounds of account book");
                continue;
            }
            let one_ab = &ab.list[index];

            let supply = if ins.data.supply.is_none() {
                u64::MAX
            } else {
                ins.data.supply.unwrap()
            };

            let new_used_num = supply - one_ab.residue;

            if new_used_num != ins.meta.used_num {
                // let mut n_ins = ins;
                ins.meta.used_num = new_used_num;
                update_arr.push(ins);
            }
        }

        // println!("update arr, {:?}", update_arr);

        for one_ins in update_arr.iter() {
            let rs = collection
                .update_one(
                    doc! {"key": one_ins.key.to_string()},
                    doc! {"$set": {"meta.used_num": one_ins.meta.used_num as i64}},
                    None,
                )
                .await?;

            if rs.modified_count != 1 {
                return Err(anyhow!("modified account is not right, {:?}", rs));
            }
        }

        // collection.update_many(doc!{}, update_arr, None).await?;

        Ok(())
    }

    */

    pub async fn get_all_ins(
        &self,
        size: usize,
        page_index: usize,
        sort_type: String,
        range_type: Option<String>,
        key: Option<String>,
        publisher: Option<String>,
        search_type: Option<String>,
        search_str: Option<String>,
        start_time_filter: Option<String>,
    ) -> Result<InscriptionList> {
        let count = self.get_inscription_count(0).await?;
        let deployed_count = self.get_inscription_count(1).await?;

        let collection = self.get_ins_coll()?;
        let mut arr = vec![];
        let size = size as u64;
        let page_index = page_index as u64;

        let skip = size * page_index;

        // let mut opts = FindOptions::builder().skip(skip).limit(size as i64);
        // if sort_type == "time" {
        //     opts = opts.sort(doc!{"data.deployed_ts":-1});
        // }

        let mut prev_arr = vec![];

        if key.is_some() {
            let regex = format!("^{}$", key.unwrap());
            prev_arr.push(doc! {
                "$match": {
                    "key":{"$regex": regex, "$options":"i"}
                }
            });
        }

        if publisher.is_some() {
            let regex = format!("^{}$", publisher.unwrap());
            prev_arr.push(doc! {
                "$match": {
                    "data.publisher":{"$regex": regex, "$options":"i"}
                }
            });
        }

        {
            prev_arr.push(doc! {
                "$match": {
                    "data.deployed": true
                }
            });

            if range_type.is_some() {
                let range_type = range_type.unwrap().to_lowercase();
                if range_type == "all" {
                    // include not deployed ins
                    prev_arr.pop();
                }
            }
        }

        if search_type.is_some() {
            let search_type = search_type.unwrap().to_lowercase();
            if search_type == "in_progress" {
                prev_arr.push(doc! {
                    "$match": {
                        "$expr": {"$lt":["$data.total_replicas","$data.supply"]}
                    }
                });
            } else if search_type == "completed" {
                prev_arr.push(doc! {
                    "$match": {
                        "$expr": {"$gte":["$data.total_replicas","$data.supply"]}
                    }
                });
            }
        }

        if search_str.is_some() {
            let regex = format!("{}", search_str.unwrap());
            prev_arr.push(doc! {
                "$match": {
                    "$or":[
                        {"key":{"$regex": regex.clone(), "$options":"i"}},
                        {"metaplex_data.name":{"$regex": regex, "$options":"i"}}
                    ]

                }
            });
        }

        if start_time_filter.is_some() {
            let start_time_str = start_time_filter.unwrap();
            let start_time = start_time_str.parse::<f64>()?;
            prev_arr.push(doc! {
                "$match": {
                    "$expr": {"$lt":[start_time,"$ins_meta_data.start_time"]}
                }
            });
        }

        // if end_time_filter.is_some() {

        // }

        let agg_op = if sort_type == "time" {
            doc! {
                "$set": {
                    "sort_value": "$data.deployed_ts"
                }
            }
        } else {
            doc! {
                "$set": {
                    "sort_value": {"$multiply":vec!["$data.replica_price", "$data.total_replicas"]}
                }
            }
        };

        prev_arr.push(agg_op);

        let mut count_prev_arr = prev_arr.clone();
        count_prev_arr.extend(vec![doc! {"$count": "count"}]);
        let count_search = collection.aggregate(count_prev_arr, None).await?;
        let query_count = get_cursor_count(count_search).await?;

        // info!("after count search");

        prev_arr.extend(vec![
            doc! {
                "$sort": {
                    "sort_value": -1
                }
            },
            doc! {
                "$skip": skip as i64
            },
            doc! {
                "$limit": size as i64
            },
        ]);
        let mut docs = collection
            .aggregate(prev_arr, None)
            .await
            .map_err(|e| anyhow!("aggregate error: {:?}", e))?;

        while let Some(rs) = docs
            .try_next()
            .await
            .map_err(|e| anyhow!("try next failed: {:?}", e))?
        {
            let doc: Bson =
                bson::from_document(rs).map_err(|e| anyhow!("from document error: {:?}", e))?;
            // println!("doc: {:?}", doc);
            let ins: Inscription =
                bson::from_bson(doc).map_err(|e| anyhow!("from bson error: {:?}", e))?;
            // println!("{:?}", ins);
            arr.push(ins);
        }

        // let docs1 = collection.find(doc!{}, opts.build()).await?;

        Ok(InscriptionList {
            list: arr,
            total: count,
            deployed_count,
            query_count: query_count,
        })
    }

    pub async fn get_replica_ins(
        &self,
        size: usize,
        page_index: usize,
        sort_type: String,
        parent_ins: Option<String>,
        minter: Option<String>,
        key: Option<String>,
    ) -> Result<ReplicaInsList> {
        let count = self.get_replica_ins_count().await?;

        let collection = self.get_replica_ins_coll()?;

        let mut arr = vec![];
        let size = size as u64;
        let page_index = page_index as u64;

        let skip = size * page_index;

        let mut prev_arr = vec![];

        let mut sub_replica_count = 0;

        if parent_ins.is_some() {
            prev_arr.push(doc! {
                "$match":{
                    "data.inscription": parent_ins.as_ref().unwrap().to_string()
                }
            });

            let mut count_prev_arr = prev_arr.clone();
            count_prev_arr.extend(vec![doc! {"$count":"count"}]);
            sub_replica_count =
                get_cursor_count(collection.aggregate(count_prev_arr, None).await?).await?;
        }

        if key.is_some() {
            let regex = format!("^{}$", key.unwrap());
            prev_arr.push(doc! {
                "$match": {
                    "key":{"$regex": regex, "$options":"i"}
                }
            })
        }

        let pre_op = {
            doc! {
                "$set":{
                    "sort_value": "$data.timestamp"
                }
            }
        };
        prev_arr.push(pre_op);

        if minter.is_some() {
            // let minter_str = minter.as_ref().unwrap().to_string();
            let regex = format!("^{}$", minter.unwrap());

            // prev_arr.push(doc! {
            //     "$match": /minter_str/i
            // });
            prev_arr.push(doc! {
                "$match": {
                    "data.minter" :{"$regex":regex, "$options":"i"}
                }
            });
        }

        let mut count_prev_arr = prev_arr.clone();
        count_prev_arr.extend(vec![doc! {"$count":"count"}]);
        let query_count =
            get_cursor_count(collection.aggregate(count_prev_arr, None).await?).await?;

        prev_arr.extend(vec![
            doc! {
                "$sort": {
                    "sort_value": -1
                }
            },
            doc! {
                "$skip": skip as i64,
            },
            doc! {
                "$limit": size as i64,
            },
        ]);

        let mut docs = collection.aggregate(prev_arr, None).await?;

        while let Some(rs) = docs
            .try_next()
            .await
            .map_err(|e| anyhow!("try next failed: {:?}", e))?
        {
            let d: Bson = bson::from_document(rs)?;
            let replica_ins: ReplicaIns =
                bson::from_bson(d).map_err(|e| anyhow!("from replicaIns bson error: {:?}", e))?;
            // println!("replica_ins: {:?}", replica_ins);
            arr.push(replica_ins);
        }

        Ok(ReplicaInsList {
            list: arr,
            total: count,
            sub_replica_count,
            query_count: query_count,
        })
    }

    pub async fn get_replica_group_ins(
        &self,
        size: usize,
        page_index: usize,
        sort_type: String,
        parent_ins: Option<String>,
        minter: Option<String>,
        key: Option<String>,
    ) -> Result<ReplicaInsGroupList> {
        // let count = self.get_replica_ins_count().await?;

        let collection = self.get_replica_ins_coll()?;

        let mut arr = vec![];
        let size = size as u64;
        let page_index = page_index as u64;

        let skip = size * page_index;

        let mut prev_arr = vec![];

        prev_arr.push(doc! {
            "$group":{"_id": {"minter":"$data.minter", "inscription":"$data.inscription"}, "count":{"$sum":1}}
        });

        let pre_op = {
            doc! {
                "$set":{
                    "minter":"$_id.minter",
                    "inscription":"$_id.inscription",
                    "sort_value": "$count"
                }
            }
        };
        prev_arr.push(pre_op);

        if parent_ins.is_some() {
            let regex = format!("^{}$", parent_ins.unwrap());
            prev_arr.push(doc! {
                "$match": {
                    "inscription":{"$regex": regex, "$options":"i"}
                }
            })
        }

        // if minter.is_some() {
        //     // let minter_str = minter.as_ref().unwrap().to_string();
        //     let regex = format!("^{}$", minter.unwrap());

        //     // prev_arr.push(doc! {
        //     //     "$match": /minter_str/i
        //     // });
        //     prev_arr.push(doc! {
        //         "$match": {
        //             "data.minter" :{"$regex":regex, "$options":"i"}
        //         }
        //     });
        // }

        let mut count_prev_arr = prev_arr.clone();
        count_prev_arr.extend(vec![doc! {"$group":{"_id":null, "count":{"$sum":1}}}]);
        let mut count_search = collection.aggregate(count_prev_arr, None).await?;
        let mut query_count = 0;
        while let Some(one_row) = count_search
            .try_next()
            .await
            .map_err(|e| anyhow!("try query count next failed: {:?}", e))?
        {
            query_count = one_row.get_i32("count").unwrap_or_default() as u64;
            break;
        }

        prev_arr.extend(vec![
            doc! {"$project":{"_id":0}},
            doc! {

                "$sort": {
                    "sort_value": -1
                }
            },
            doc! {
                "$skip": skip as i64,
            },
            doc! {
                "$limit": size as i64,
            },
        ]);

        let mut docs = collection.aggregate(prev_arr, None).await?;

        while let Some(rs) = docs
            .try_next()
            .await
            .map_err(|e| anyhow!("try next failed: {:?}", e))?
        {
            let d: Bson = bson::from_document(rs)?;
            // println!("{:?}", d);
            let replica_ins: OneGroupReplicaIns = bson::from_bson(d)
                .map_err(|e| anyhow!("from GroupReplicaIns bson error: {:?}", e))?;
            // println!("replica_ins: {:?}", replica_ins);
            arr.push(replica_ins);
        }

        Ok(ReplicaInsGroupList {
            list: arr,
            total: query_count,
            query_count: query_count,
        })
    }

    pub async fn get_replica_txs(
        &self,
        size: usize,
        page_index: usize,
        sort_type: String,
    ) -> Result<Vec<dao::ReplicaTx>> {
        let collection = self.get_replica_tx_coll()?;

        let mut arr = vec![];
        let size = size as u64;
        let page_index = page_index as u64;

        let skip = size * page_index;

        let pre_op = {
            doc! {
                "$set":{
                    "sort_value": "$replica_ins.data.timestamp"
                }
            }
        };

        let mut docs = collection
            .aggregate(
                vec![
                    pre_op,
                    doc! {
                        "$sort": {
                            "sort_value": -1
                        }
                    },
                    doc! {
                        "$skip": skip as i64
                    },
                    doc! {
                        "$limit": size as i64
                    },
                ],
                None,
            )
            .await?;

        while let Some(doc) = docs
            .try_next()
            .await
            .map_err(|e| anyhow!("try next failed: {:?}", e))?
        {
            let bson: Bson = bson::from_document(doc)?;
            let d: ReplicaTx = bson::from_bson(bson)?;

            // println!("replicaIns: {:?}", d);
            arr.push(d);
        }

        Ok(arr)
    }

    pub async fn get_social_info(
        &self,
        size: usize,
        page_index: usize,
        sort_type: String,
        key: Option<String>,
    ) -> Result<SocialInfoDB> {
        let collection = self.get_social_info_coll()?;
        if key.is_none() {
            return Err(anyhow!("key is required"));
        }
        let key = key.unwrap();

        let doc_opt = collection.find_one(doc! {"key": key}, None).await?;
        if doc_opt.is_none() {
            return Ok(SocialInfoDB::default());
        }

        Ok(doc_opt.unwrap())
    }

    pub async fn save_social_info(
        &self,
        size: usize,
        page_index: usize,
        sort_type: String,
        social_info: SocialInfoDB,
    ) -> Result<()> {
        let collection = self.get_social_info_coll()?;

        collection
            .delete_one(doc! {"key": social_info.key.to_string()}, None)
            .await?;

        let _ = collection.insert_one(social_info, None).await?;

        Ok(())
    }
}

pub fn escape_c(s: &str) -> String {
    s.replace("'", "").replace("\\", "")
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use crate::{
        common::{dao::InsMetaData, opt_pubkey_serde, pubkey_serde},
        config::application::Application,
    };

    use super::*;

    #[derive(Debug, Serialize, Deserialize, Clone)]
    enum Category {
        One,
        Twe,
    }

    #[derive(Debug, Serialize, Deserialize, Clone)]
    struct Test {
        #[serde(with = "opt_pubkey_serde")]
        key: Option<Pubkey>,
        pub ins_meta_data: Option<InsMetaData>,
        // opt: Option<u8>,
        index: u64,
    }

    const pk: &str = "13MBi6yjdRL87VeraMAaGooyXTHyZiEicXC844eqHBki";

    #[tokio::test]
    async fn test_count() -> Result<()> {
        util::set_log_conf();
        let application = Application::new().unwrap();
        global_var::init_app_conf(application);
        let db_ins = create_db_ins().await?;
        set_db_ins(db_ins);
        let db_ins = get_db_ins();
        let collection = db_ins.get_ins_coll()?;
        // db_ins.update_all_ins_used_num(true).await?;
        let g_name = "$group";
        let count_name = "count";
        let mut count_prev_arr = vec![];

        let mut d = doc! {"a":[], "$match":{"$or":[]}};
        d.get_array_mut("a").unwrap().push(3.1.into());
        d.insert("b", "234");
        d.insert("c", 21u32);
        d.insert("d", 32.1);
        d.insert("$group", Bson::Array(vec!["34".into()]));
        d.get_document_mut("$match")
            .unwrap()
            .get_array_mut("$or")
            .unwrap()
            .push(Bson::String("234".to_string()));

        println!("doc: {:?}", d);
        let v = "1";
        println!("{}", v[2..].len());
        return Ok(());

        count_prev_arr.extend(vec![doc! {g_name:{"_id":null, count_name:{"$sum":1}}}]);
        let mut count_search = collection.aggregate(count_prev_arr, None).await?;
        let mut query_count = 0;
        while let Some(one_row) = count_search
            .try_next()
            .await
            .map_err(|e| anyhow!("try query count next failed: {:?}", e))?
        {
            println!("one_row: {:?}", one_row);
            query_count = one_row.get_i32("count").unwrap_or_default() as u64;
            break;
        }

        println!("query_count: {}", query_count);

        Ok(())
    }

    #[tokio::test]
    async fn test() -> Result<()> {
        let application = Application::new().unwrap();
        global_var::init_app_conf(application);
        let db_ins = create_db_ins().await?;
        set_db_ins(db_ins);
        let db_ins = get_db_ins();
        let coll = db_ins.db.collection::<Test>("test");

        let mut i = 0;

        let mut ins_meta = InsMetaData::default();
        ins_meta.start_time = 11235;

        coll.insert_one(
            Test {
                key: None,
                // opt: Some(33),
                ins_meta_data: Some(ins_meta),
                index: i,
            },
            None,
        )
        .await?;
        // return Ok(());

        // println!("{}", pk.to_string());
        let v = 33;

        let mut cursor = coll
            .aggregate(vec![doc! {"$match":{"index": 0}}], None)
            .await?;

        while let Some(t) = cursor.try_next().await? {
            println!("{:?}", t);
        }

        Ok(())
    }
}
