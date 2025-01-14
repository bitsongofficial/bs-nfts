import codegen from '@cosmwasm/ts-codegen';

codegen({
  contracts: [
    {
      name: 'Bs721Account',
      dir: '../contracts/collections/bs721-accounts/schema'
    },
    {
      name: 'Bs721Base',
       dir: '../contracts/collections/bs721-base/schema'
    },
    {
      name: 'Bs721Curve',
       dir: '../contracts/collections/bs721-curve/schema'
    },
    {
      name: 'Bs721Royalties',
       dir: '../contracts/collections/bs721-royalties/schema'
    },
    {
      name: 'Bs721AccountMarketplace',
       dir: '../contracts/markets/account-market/schema'
    },
    {
      name: 'Bs721AccountMarketplace',
       dir: '../contracts/minters/account-minter/schema'
    },
    {
      name: 'Bs721AccountMarketplace',
       dir: '../contracts/minters/bs721-launchparty/schema'
    }
  ],
  outPath: './src/',

  // options are completely optional ;)
  options: {
    bundle: {
      bundleFile: 'bundle.ts',
      scope: 'contracts'
    },
    types: {
      enabled: true
    },
    client: {
      enabled: true
    },
    reactQuery: {
      enabled: false,
      optionalClient: true,
      version: 'v4',
      mutations: true,
      queryKeys: true,
      queryFactory: true,
    },
    recoil: {
      enabled: false
    },
    messageComposer: {
        enabled: true
    },
  }
}).then(() => {
  console.log('✨ all done!');
});