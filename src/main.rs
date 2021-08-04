use actix;

use chrono::{NaiveDateTime, DateTime}; 
use chrono::prelude::*; 

use near_indexer::near_primitives::views::ExecutionStatusView;
use near_client::ViewClientActor;

use clap::Clap;
use tokio::sync::mpsc;

use actix::Addr;

use configs::{init_logging, Opts, SubCommand};
use near_indexer;

use serde::{Serialize, Deserialize};

mod configs;

static CONTRACT_TO_VIEW: &str = "market2.test.near"; 

//use this struct to store information that we want to pass to database
#[derive(Debug)] //derive debug so that we can print
struct ExecutionDetails {
    method_name: String,
    predecessor_id: String,
    receiver_id: String, 
    signer_id: String,
    function_call_deposit: String,
    transfer_deposit: String,
    success_value: bool,
    date: String, 
}

//declare struct for the return type of the blockchain view call
#[derive(Serialize, Deserialize, Debug)]
pub struct JsonToken {
    pub owner_id: String, //only declaring the field we care about
}

async fn listen_blocks(mut stream: mpsc::Receiver<near_indexer::StreamerMessage>, view_client: Addr<ViewClientActor>) {
    eprintln!("listen_blocks");
    //listen for streams
    while let Some(streamer_message) = stream.recv().await {
        //iterate through each shard in the incoming stream
        for shard in streamer_message.shards {
            //for each receipt and execution outcome pair in the shard
            for receipt_and_execution_outcome in shard.receipt_execution_outcomes {
                // Check if receipt is related to Fayyr
                if is_contract_receipt(&receipt_and_execution_outcome.receipt) {
                    //get the execution outcome from the receipt and execution outcome pair from the shard
                    let execution_outcome = receipt_and_execution_outcome.execution_outcome;
                    let receipt = receipt_and_execution_outcome.receipt;

                    //declare the execution details that will hold specific wanted information
                    let mut execution_details = ExecutionDetails {
                        method_name: "".to_string(),
                        predecessor_id: "".to_string(),
                        receiver_id: "".to_string(),
                        signer_id: "".to_string(),
                        function_call_deposit: "".to_string(),
                        transfer_deposit: "".to_string(),
                        success_value: matches!(execution_outcome.outcome.status, ExecutionStatusView::SuccessValue(_) | ExecutionStatusView::SuccessReceiptId(_)),
                        date: "".to_string(),
                    };
                    let nano_to_seconds = 1e+9 as u64; 
                    let seconds = (streamer_message.block.header.timestamp / nano_to_seconds) as i64; 


                    let naive = NaiveDateTime::from_timestamp(seconds, 0); 
                    let datetime: DateTime<Utc> = DateTime::from_utc(naive, Utc); 
                    let newdate = datetime.format("%Y-%m-%d %H:%M:%S");

                    execution_details.date = newdate.to_string(); 

                    //get the signer id from the receipt
                    let signer_id = if let near_indexer::near_primitives::views::ReceiptEnumView::Action { ref signer_id, .. } = receipt.receipt {
                        Some(signer_id)
                    } else {
                        None
                    };

                    //if there is some signer id, set the execution detail's signer equal to it
                    match signer_id {
                        Some(signer_id) => {
                            execution_details.signer_id = signer_id.to_string();
                        },
                        _ => {},
                    };

                    //get the predecessor_id from the receipt
                    let predecessor_id = if let near_indexer::near_primitives::views::ReceiptView { ref predecessor_id, .. } = receipt {
                        Some(predecessor_id)
                    } else {
                        None
                    };

                    //if there is some predecessor_id, set the execution detail's predecessor_id equal to it
                    match predecessor_id {
                        Some(predecessor_id) => {
                            execution_details.predecessor_id = predecessor_id.to_string();
                        },
                        _ => {},
                    };

                    //get the receiver_id from the receipt
                    let receiver_id = if let near_indexer::near_primitives::views::ReceiptView { ref receiver_id, .. } = receipt {
                        Some(receiver_id)
                    } else {
                        None
                    };

                    //if there is some receiver_id, set the execution detail's receiver_id equal to it
                    match receiver_id {
                        Some(receiver_id) => {
                            execution_details.receiver_id = receiver_id.to_string();
                        },
                        _ => {},
                    };

                    //get the actions from the receipt
                    if let near_indexer::near_primitives::views::ReceiptEnumView::Action {
                        actions,
                        ..
                    } = receipt.receipt
                    {   
                        //go through each action
                        for action in actions.iter() {
                            //get the method name 
                            if let near_indexer::near_primitives::views::ActionView::FunctionCall {
                                method_name,
                                ..
                            } = action
                            {
                                execution_details.method_name = method_name.to_string();
                            }
                            //get the deposit
                            if let near_indexer::near_primitives::views::ActionView::FunctionCall {
                                deposit,
                                ..
                            } = action
                            {
                                execution_details.function_call_deposit = deposit.to_string();
                            }
                            //get the transfer deposit if there is some
                            if let near_indexer::near_primitives::views::ActionView::Transfer {
                                deposit,
                                ..
                            } = action
                            {
                                execution_details.transfer_deposit = deposit.to_string();
                            }
                        }
                    }

                    //only do stuff with there is a deposit
                    if execution_details.function_call_deposit != "" {
                        if execution_details.receiver_id == CONTRACT_TO_VIEW {
                            eprintln!("{} Deposited INTO Contract. Full Details --> {:?}\n\n\n\n", execution_details.function_call_deposit, execution_details);
                        } else if execution_details.signer_id == CONTRACT_TO_VIEW || execution_details.predecessor_id == CONTRACT_TO_VIEW {
                            eprintln!("{} SPENT By Contract. Full Details --> {:?}\n\n\n\n", execution_details.function_call_deposit, execution_details);
                        } else {
                            eprintln!("Not Sure What Happened --> {:?}\n\n\n\n", execution_details);
                        }
                    } else if execution_details.transfer_deposit != "" {
                        if execution_details.receiver_id == CONTRACT_TO_VIEW {
                            eprintln!("{} Deposited INTO Contract. Full Details --> {:?}\n\n\n\n", execution_details.transfer_deposit, execution_details);
                        } else if execution_details.signer_id == CONTRACT_TO_VIEW || execution_details.predecessor_id == CONTRACT_TO_VIEW {
                            eprintln!("{} SPENT By Contract. Full Details --> {:?}\n\n\n\n", execution_details.transfer_deposit, execution_details);
                        } else {
                            eprintln!("Not Sure What Happened --> {:?}\n\n\n\n", execution_details);
                        }
                    } else {
                        eprintln!("No Transfer Of Funds. Full Details --> {:?}\n\n\n\n", execution_details);
                    }
                }
            }
        }
    }
}

// Assuming contract deployed to account id market.test.near
// Checks if receipt receiver is equal to the account id we care about
fn is_contract_receipt(receipt: &near_indexer::near_primitives::views::ReceiptView) -> bool {
    receipt.receiver_id.as_str() == CONTRACT_TO_VIEW || receipt.predecessor_id.as_str() == CONTRACT_TO_VIEW
}

fn main() {
    // We use it to automatically search the for root certificates to perform HTTPS calls
    // (sending telemetry and downloading genesis)
    openssl_probe::init_ssl_cert_env_vars();
    init_logging();

    let opts: Opts = Opts::parse();

    let home_dir = opts
        .home_dir
        .unwrap_or(std::path::PathBuf::from(near_indexer::get_default_home()));

    match opts.subcmd {
        //if we run cargo run -- run
        SubCommand::Run => {
            eprintln!("Starting...");

            //get the indexer config from the home directory
            let indexer_config = near_indexer::IndexerConfig {
                home_dir,
                //Starts syncing from the block by which the indexer was interupted the las time it was run
                sync_mode: near_indexer::SyncModeEnum::BlockHeight(0),
                //wait until the entire syncing process is finished before streaming starts. 
                await_for_node_synced: near_indexer::AwaitForNodeSyncedEnum::StreamWhileSyncing, 
            };

            let sys = actix::System::new();
            sys.block_on(async move {
                eprintln!("Actix");
                let indexer = near_indexer::Indexer::new(indexer_config);
                //use view client to make view calls to the blockchain
                let view_client = indexer.client_actors().0; //returns tuple, second is another client actor - we only care about first value
                let stream = indexer.streamer();
                actix::spawn(listen_blocks(stream, view_client));
            });
            sys.run().unwrap();
        }
        //if we run cargo run -- init
        //initialize configs in the home directory (~./near)
        SubCommand::Init(config) => near_indexer::indexer_init_configs(&home_dir, config.into()),
    }
}