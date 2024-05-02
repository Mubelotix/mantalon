use crate::*;

pub async fn relay_websocket_to_transport(mut receiver: WsReceiver, mut writer: Box<dyn AsyncWrite + Send + Unpin>) {
    let mut message = Vec::new();
    loop {
        message.clear();
        match receiver.receive_data(&mut message).await {
            Ok(Data::Binary(n)) => {
                assert_eq!(n, message.len());
                writer.write_all(&message).await.unwrap();
                writer.flush().await.unwrap();
            }
            Ok(Data::Text(n)) => {
                assert_eq!(n, message.len());
                writer.write_all(&message).await.unwrap();
                writer.flush().await.unwrap();
            }
            Err(SockettoError::Closed) => break,
            Err(e) => {
                error!("Websocket connection error: {e}");
                break;
            }
        }
    }
}

#[allow(clippy::uninit_vec)]
pub async fn relay_transport_to_websocket(mut reader: Box<dyn AsyncRead + Send + Unpin>, mut sender: WsSender) {
    let mut buffer = Vec::with_capacity(100_000);
    unsafe {
        buffer.set_len(buffer.capacity());
    }
    loop {
        let n = match reader.read(&mut buffer).await {
            Ok(n) => n,
            Err(e) => {
                error!("Transport read error: {e}");
                break;
            }
        };
        if n == 0 {
            break;
        }
        sender.send_binary(&buffer[..n]).await.unwrap();
        sender.flush().await.unwrap();
    }
}
