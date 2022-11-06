use ev3dev_lang_rust::{
    motors::{MotorPort, TachoMotor},
    Ev3Result,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Deserialize, Debug)]
pub struct FinalizedHead {
    params: Params,
}

#[derive(Deserialize, Debug)]
pub struct Params {
    result: Result,
}

#[derive(Deserialize, Debug)]
pub struct Result {
    number: String,
}

const CHAIN_ENDPOINT: &str = "wss://ws-node-gm.terrabiodao.org";
const GEAR_RATIO: f32 = 0.33;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Ev3Result<()> {
    let motor = TachoMotor::get(MotorPort::OutA)?;

    motor.set_stop_action("hold")?;
    motor.set_position(0)?;
    motor.set_speed_sp(500)?;

    let url = url::Url::parse(CHAIN_ENDPOINT).unwrap();

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

    let count_per_rot = ((motor.get_count_per_rot()? as f32) / GEAR_RATIO).round() as i32;

    read.for_each(|message| async {
        let data = message.unwrap().into_data();

        if let Ok(res) = serde_json::from_slice::<FinalizedHead>(&data) {
            if let Ok(block_number) =
                i32::from_str_radix(res.params.result.number.trim_start_matches("0x"), 16)
            {
                println!("Block number: {}", block_number);

                if let Ok(current_position) = motor.get_position() {
                    let new_position: i32 = (count_per_rot * (block_number % 2760)) / 2760;

                    let difference = new_position - current_position;
                    let difference_modulo = if (difference) < 0 {
                        -difference
                    } else {
                        difference
                    };

                    println!("difference_modulo: {}", difference_modulo);

                    if difference_modulo > 3 {
                        println!("Old position: {}", current_position);
                        println!("New position: {}", new_position);
                        if let Err(e) = motor.run_to_abs_pos(Some(new_position)) {
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
