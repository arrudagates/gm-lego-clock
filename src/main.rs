use std::cmp::Ordering;

use ev3dev_lang_rust::{
    motors::{MotorPort, TachoMotor},
    Ev3Result,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Serialize, Deserialize, Debug)]
pub struct FinalizedHead {
    params: Params,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Params {
    result: Result,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Result {
    number: String,
}

fn run_motor_to(motor: TachoMotor, to: i32) -> Ev3Result<()> {
    loop {
        let pos = motor.get_position()?;

        if ((pos - to) as u32) > 5 {
            motor.run_direct()?;

            match pos.cmp(&to) {
                Ordering::Less => motor.set_duty_cycle_sp(40)?,
                Ordering::Greater => motor.set_duty_cycle_sp(-40)?,
                Ordering::Equal => {
                    motor.set_duty_cycle_sp(0)?;
                    motor.stop()?;

                    eprintln!("current pos: {}", pos);

                    return Ok(());
                }
            }
        } else {
            motor.set_duty_cycle_sp(0)?;
            motor.stop()?;

            eprintln!("current pos: {}", pos);

            return Ok(());
        }
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Ev3Result<()> {
    let motor = TachoMotor::get(MotorPort::OutA)?;

    motor.set_stop_action("hold")?;

    motor.set_position(0)?;

    let connect_addr = "wss://ws-node-gm.terrabiodao.org";

    let url = url::Url::parse(&connect_addr).unwrap();

    let (mut ws_stream, _) = connect_async(url).await.expect("Failed to connect");

    ws_stream
        .send(Message::text(
            "{\"id\":1, \"jsonrpc\":\"2.0\", \"method\": \"chain_subscribeFinalizedHeads\"}"
                .to_string(),
        ))
        .await
        .unwrap();

    let (_, read) = ws_stream.split();

    let motor = TachoMotor::get(MotorPort::OutA)?;

    let count_per_rot = motor.get_count_per_rot()?;

    read.for_each(|message| async {
        let data = message.unwrap().into_data();

        if let Ok(res) = serde_json::from_slice::<FinalizedHead>(&data) {
            if let Ok(block_number) =
                i32::from_str_radix(res.params.result.number.trim_start_matches("0x"), 16)
            {
                println!("Block number: {}", block_number);

                if let Ok(current_position) = motor.get_position() {
                    let new_position: i32 = (count_per_rot * (block_number % 2760)) / 2760;

                    if new_position != current_position {
                        println!("New position: {}", new_position);
                        if let Err(e) = run_motor_to(motor.clone(), new_position) {
                            println!("Error running to position: {:?}", e);
                        }
                    }
                }
            }
        };
    })
    .await;

    Ok(())
}
