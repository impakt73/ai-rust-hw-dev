use bus_shared::{
    classify_request_region, request_end_addr, BusRequest, BusResponse, HandlerError,
    HostBusHandler, RequestAddressRegion,
};

/// Restricted view of simulator bus plumbing used by the interactive runtime.
pub struct SimulatorView<'a> {
    bus: &'a mut bus_shared::SystemBus,
    host_bus_handler: &'a mut HostBusHandler,
    direct_response: &'a mut Option<BusResponse>,
}

impl<'a> SimulatorView<'a> {
    pub(crate) fn new(
        bus: &'a mut bus_shared::SystemBus,
        host_bus_handler: &'a mut HostBusHandler,
        direct_response: &'a mut Option<BusResponse>,
    ) -> Self {
        SimulatorView {
            bus,
            host_bus_handler,
            direct_response,
        }
    }

    pub fn send_bus_request(&mut self, request: BusRequest) -> Result<(), String> {
        if request_end_addr(&request).is_none() {
            return Err(format!(
                "Host request rejected: {:?}",
                HandlerError::InvalidAddressRange
            ));
        }

        if self.host_bus_handler.has_pending_outgoing_request() || self.direct_response.is_some() {
            return Err(format!(
                "Host request rejected: {:?}",
                HandlerError::RequestPending
            ));
        }

        match classify_request_region(&request) {
            RequestAddressRegion::RtlPeripheral => self
                .host_bus_handler
                .send_request(request)
                .map_err(|e| format!("Host request rejected: {:?}", e)),
            RequestAddressRegion::NonRtl => {
                let beat_bytes = usize::from(request.size.byte_count());
                let beats = request.burst_len.max(1);
                let response = if request.we {
                    for beat_idx in 0..beats {
                        let addr = if request.dst_fixed {
                            request.addr
                        } else {
                            request
                                .addr
                                .checked_add(
                                    beat_idx * u32::try_from(beat_bytes).expect("beat bytes fit"),
                                )
                                .expect("request range pre-validated")
                        };
                        let byte_offset = (beat_idx as usize) * beat_bytes;
                        let mut beat_buf = [0u8; 4];
                        if request.data.len() >= byte_offset + beat_bytes {
                            beat_buf[..beat_bytes].copy_from_slice(
                                &request.data[byte_offset..byte_offset + beat_bytes],
                            );
                        } else if beat_idx == 0 {
                            beat_buf = request.wdata.to_le_bytes();
                        }
                        let wdata = u32::from_le_bytes(beat_buf);

                        match request.size {
                            bus_shared::AccessSize::Byte => self.bus.write_byte(addr, wdata as u8),
                            bus_shared::AccessSize::Halfword => {
                                self.bus.write_halfword(addr, wdata as u16)
                            }
                            bus_shared::AccessSize::Word => self.bus.write_word(addr, wdata),
                        }
                    }
                    let mut response = BusResponse::write_ack(request.size);
                    response.addr = request.addr;
                    response.burst_len = request.burst_len;
                    response.src_fixed = request.src_fixed;
                    response.dst_fixed = request.dst_fixed;
                    response
                } else {
                    let mut burst_data = Vec::with_capacity((beats as usize) * beat_bytes);
                    for beat_idx in 0..beats {
                        let addr = if request.src_fixed {
                            request.addr
                        } else {
                            request
                                .addr
                                .checked_add(
                                    beat_idx * u32::try_from(beat_bytes).expect("beat bytes fit"),
                                )
                                .expect("request range pre-validated")
                        };
                        let rdata = match request.size {
                            bus_shared::AccessSize::Byte => self.bus.read_byte(addr) as u32,
                            bus_shared::AccessSize::Halfword => self.bus.read_halfword(addr) as u32,
                            bus_shared::AccessSize::Word => self.bus.read_word(addr),
                        };
                        burst_data.extend_from_slice(&rdata.to_le_bytes()[..beat_bytes]);
                    }
                    BusResponse::burst_read_data(
                        request.addr,
                        request.size,
                        request.burst_len,
                        request.src_fixed,
                        request.dst_fixed,
                        burst_data,
                    )
                };
                *self.direct_response = Some(response);
                Ok(())
            }
            RequestAddressRegion::SpansRtlBoundary => Err(format!(
                "Host request rejected: {:?}",
                HandlerError::InvalidAddressRange
            )),
        }
    }

    pub fn receive_bus_response(&mut self) -> Option<BusResponse> {
        self.direct_response
            .take()
            .or_else(|| self.host_bus_handler.receive_response())
    }
}
