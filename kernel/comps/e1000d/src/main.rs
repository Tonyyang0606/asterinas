#![no_std]
#![deny(unsafe_code)]
#![feature(strict_provenance)]

extern crate alloc;
use alloc::boxed::Box;
use alloc::sync::Arc;
use aster_virtio::device::socket::buffer;
use aster_virtio::device::network::header::VirtioNetHdr;
use ostd::mm;
use ostd::mm::{VmReader,DmaDirection,DmaStream,VmWriter};
use ostd::prelude::println;
use ostd::bus::pci::common_device::PciCommonDevice;
use ostd::sync::{Mutex,SpinLock,LocalIrqDisabled};
use ostd::bus::BusProbeError;
use ostd::bus::pci::PCI_BUS;
use ostd::bus::pci::bus::PciDevice;
use ostd::bus::pci::bus::PciDriver;
use ostd::bus::pci::cfg_space::Bar;
use alloc::{vec::Vec};
use component::{init_component, ComponentInitError};
use ostd::bus::pci::PciDeviceId;
use aster_network::{DmaSegment, RxBuffer, TxBuffer,AnyNetworkDevice,VirtioNetError,EthernetAddr};
use aster_bigtcp::device::DeviceCapabilities;
use core::fmt::Debug;
use alloc::fmt;
use alloc::collections::linked_list::LinkedList;
use aster_network::dma_pool;


#[init_component]
fn e1000_init() -> Result<(), ComponentInitError> {
    driver_e1000_init();
    Ok(())
}

/// The dma descriptor for transmitting
#[derive(Debug, Clone)]
#[repr(C, align(16))]
pub struct TD {
addr: u64,
length: usize,
cso: u8,
cmd: u8,
status: u8,
css: u8,
special: u16,
}
/// [E1000 3.2.3]
/// The dma descriptor for receiving
#[derive(Debug, Clone)]
#[repr(C, align(16))]
pub struct RD {
addr: u64, /* Address of the descriptor's data buffer */
length: usize, /* Length of data DMAed into data buffer */
csum: u16, /* Packet checksum */
status: u8, /* Descriptor status */
errors: u8, /* Descriptor Errors */
special: u16,
}

pub struct PciDeviceE1000 {
    common_device: PciCommonDevice,
    base: usize,
    mac_address: EthernetAddr,
    header: VirtioNetHdr,
    caps: DeviceCapabilities,
    receive_buffers: Vec<Arc<RxBuffer>>,
    receive_ring: Vec<Arc<RD>>,
    receive_index: usize,

    transmit_buffers: Vec<Arc<TxBuffer>>,
    transmit_ring: Vec<Arc<TD>>,
    //transmit_ring_free: usize,
    transmit_index: usize,
    transmit_clean_index: usize,

    dma_pool_device: Arc<dma_pool::DmaPool>
}
impl fmt::Debug for PciDeviceE1000 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PciDeviceE1000")
            .field("common_device", &self.common_device)
            .field("base", &self.base)
            .field("mac_address", &self.mac_address)
            .field("cap", &self.caps)
            .field("header", &self.header)
            .field("receive_index", &self.receive_index)
            .field("transmit_index", &self.transmit_index)
            .field("transmit_clean_index", &self.transmit_clean_index)
            .finish()
    }
}
/* 
impl AnyNetworkDevice for PciDeviceE1000 {
    fn mac_addr(&self) -> EthernetAddr {
        self.mac_address
    }
    fn capabilities(&self) -> DeviceCapabilities {
        self.caps
        }
    
    }*/


impl PciDeviceE1000{
    fn mac_addr(&self) -> EthernetAddr {
        self.mac_address
    }
    pub fn new(common_device: PciCommonDevice,mac_address:EthernetAddr) -> Self{
        let dma_pool_new = dma_pool::DmaPool::new(
            mm::PAGE_SIZE,
            10,
            50,
            DmaDirection::Bidirectional,
            false,
        );
        
        Self{
            common_device,
            base: 0,
            mac_address: aster_network::EthernetAddr([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
            header: VirtioNetHdr::default(),
            caps: DeviceCapabilities::default(),
            receive_ring: Vec::with_capacity(64),
            receive_buffers: Vec::with_capacity(64),
            receive_index: 0,

            transmit_ring: Vec::with_capacity(64),
            transmit_buffers: Vec::with_capacity(64),
            transmit_index: 0,
            transmit_clean_index: 0,

            dma_pool_device: dma_pool_new,
        }
    }
   /*  pub fn send_packet(&mut self, packet: &[u8]) -> Result<(), VirtioNetError>{
        if self.transmit_index >= self.transmit_buffers.len() {
            return Err(VirtioNetError::Busy);
        }
        let segment = self.dma_pool_device.alloc_segment().unwrap();
        let mut writer = segment.writer().unwrap();
        let buffer = TxBuffer::new(
            self.header,
            packet,
            &TX_BUFFER_POOL
        );
        // Ensure the TxBuffer can be transformed to bytes stream
        writer.write(&mut VmReader::from(TxBuffer));
        let buffer_arc = Arc::new(buffer);
        writer.write(&mut VmReader::from(buffer_arc));
        self.transmit_buffers[self.transmit_index] = buffer_arc;
        let td = TD{
            addr: std::ptr::addr_of!(buffer),  // 填入适当的值
            length: packet.len(),
            cso: 0,
            cmd: 0,
            status: 0,
            css: 0,
            special: 0,
        };
        writer.write(&mut VmReader::from(td));
        let td_arc = Arc::new(td);
        writer.write(&mut VmReader::from(td_arc));
        self.transmit_ring[self.transmit_index] = td_arc;
        self.transmit_index += 1;
        Ok(())
        //TODO: Implement notify the device when the data is ready.
    }
    
    pub fn receive_packet(&mut self) -> Result<Vec<u8>, VirtioNetError>{
        let buffer = self.receive_buffers[self.receive_index].as_ref();
        let rd = self.receive_ring[self.receive_index].as_ref();
        let mut reader = (&buffer::segment).reader().unwrap();
        let packet = [0u8; rd.length];
        reader.read(&mut VmWriter::from(&mut packet as &mut [u8]));
        self.receive_index = (self.receive_index + 1) % 64;
        Ok(packet)
        //TODO: Implement notify the receive end when the data arrive.
    }*/
}
static TX_BUFFER_POOL: SpinLock<LinkedList<DmaStream>, LocalIrqDisabled> =
    SpinLock::new(LinkedList::new());
    
impl PciDevice for PciDeviceE1000 {
    fn device_id(&self) -> PciDeviceId {
        self.common_device.device_id().clone()
    }
}

#[derive(Debug)]
pub struct PciDriverE1000 {
    devices: Mutex<Vec<Arc<PciDeviceE1000>>>,
}

impl PciDriver for PciDriverE1000 {
    fn probe(
        &self,
        device: PciCommonDevice
    ) -> Result<Arc<dyn PciDevice>, (BusProbeError, PciCommonDevice)> {
        // 检查设备是否匹配
        if device.device_id().vendor_id != 0x8086 || device.device_id().device_id != 0x100E {
            // 0x8086 是 Intel 的 Vendor ID，0x100E 是 e1000 的 Device ID
            return Err((BusProbeError::DeviceNotMatch, device));
        }
        // 创建 DMA 池
        let dma_pool_new = dma_pool::DmaPool::new(
            mm::PAGE_SIZE,
            10,
            50,
            DmaDirection::Bidirectional,
            false,
        );

        // 获取设备的 MAC 地址，假设可以从设备配置空间读取
      

        // 创建 PciDeviceE1000 的实例
        let pci_device = Arc::new(PciDeviceE1000 {
            common_device: device,
            base: 0,
            mac_address: aster_network::EthernetAddr([0x52, 0x54, 0x00, 0x12, 0x34, 0x56]),
            header: VirtioNetHdr::default(),
            caps: DeviceCapabilities::default(),
            receive_ring: Vec::with_capacity(64),
            receive_buffers: Vec::with_capacity(64),
            receive_index: 0,

            transmit_ring: Vec::with_capacity(64),
            transmit_buffers: Vec::with_capacity(64),
            transmit_index: 0,
            transmit_clean_index: 0,

            dma_pool_device: dma_pool_new,
        });

        // 将设备添加到驱动的设备列表
        println!("{:?}",pci_device);
        self.devices.lock().push(pci_device.clone());

        // 返回创建的设备
        Ok(pci_device)
    }
}


pub fn driver_e1000_init() {
    let driver_a = Arc::new(PciDriverE1000 {
        devices: Mutex::new(Vec::new()),
    });
    PCI_BUS.lock().register_driver(driver_a);
}
